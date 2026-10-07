package net.havenkeys.android.ui.health

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.HealthCountsView
import uniffi.havenkeys_mobile.HealthIssueView
import uniffi.havenkeys_mobile.HealthKind

sealed interface HealthFilter {
    data object All : HealthFilter
    data object Dismissed : HealthFilter
    data class Kind(val kind: HealthKind) : HealthFilter
}

/**
 * One row: the title and username from the overview, never a value.
 * [busy]: a Dismiss or Undo on this login is still being saved.
 */
data class HealthRow(
    val id: String,
    val title: String,
    val subtitle: String?,
    val kinds: List<HealthKind>,
    val groupSize: Int?,
    val duplicates: Int?,
    val dismissed: Boolean,
    val busy: Boolean = false,
)

/** Ids, check kinds, counts and overviews only (spec §8); all of it is dropped on lock. */
data class HealthUiState(
    val counts: HealthCountsView? = null,
    val total: Int = 0,
    val filter: HealthFilter = HealthFilter.All,
    val rows: List<HealthRow> = emptyList(),
    val loading: Boolean = true,
    val errorCode: String? = null,
)

/** Watchtower's order, then "old". */
val KIND_ORDER = listOf(
    HealthKind.REUSED, HealthKind.WEAK, HealthKind.INSECURE, HealthKind.DUPLICATE,
    HealthKind.PASSKEY, HealthKind.TWO_FACTOR, HealthKind.OLD,
)

fun HealthCountsView.of(kind: HealthKind): Int = when (kind) {
    HealthKind.WEAK -> weak
    HealthKind.REUSED -> reused
    HealthKind.OLD -> old
    HealthKind.PASSKEY -> passkey
    HealthKind.TWO_FACTOR -> twoFactor
    HealthKind.INSECURE -> insecure
    HealthKind.DUPLICATE -> duplicate
}.toInt()

// Rust numbers groups before dismissals, so a group's other logins may not all
// be among the shown issues: the chips never claim fewer than the check implies.

/** "Used in N logins" for a reused group with [shown] logins on screen: at least 2. */
internal fun reusedIn(shown: Int): Int = maxOf(2, shown)

/** "Duplicate of N other logins" for a duplicate group with [shown] logins on screen: at least 1. */
internal fun duplicatesOf(shown: Int): Int = maxOf(1, shown - 1)

/** The chips' counts for one login of a report: its group sizes among the report's active issues. */
internal fun chipCounts(issue: HealthIssueView, issues: List<HealthIssueView>): Pair<Int?, Int?> {
    val active = issues.filter { !it.dismissed }
    val reused = issue.reusedGroup?.let { g ->
        reusedIn(active.count { it.reusedGroup == g && HealthKind.REUSED in it.kinds })
    }
    val duplicates = issue.duplicateGroup?.let { g ->
        duplicatesOf(active.count { it.duplicateGroup == g && HealthKind.DUPLICATE in it.kinds })
    }
    return reused to duplicates
}

/** Every check counted once per login, as the Home card and the screen show it. */
fun HealthCountsView.total(): Int = KIND_ORDER.sumOf { of(it) }

/**
 * The Health screen's report. It loads when the screen shows and when items
 * change, never from typing; neither counts as user activity for auto-lock.
 */
class HealthViewModel(private val vault: VaultRepository, events: VaultEventsHub) : ViewModel() {
    private val _state = MutableStateFlow(HealthUiState())
    val state: StateFlow<HealthUiState> = _state.asStateFlow()
    private var issues: List<HealthIssueView> = emptyList()
    private var titles: Map<String, Pair<String, String?>> = emptyMap()
    private var pending: Job? = null

    /**
     * The dismissed lists saved since the current report, by login. The command
     * replaces a login's whole list, so the next Dismiss or Undo builds on what
     * was last saved; a report whose load began after the save supersedes it.
     */
    private val saved = mutableMapOf<String, Saved>()

    /** Logins with a change in flight (not yet saved). */
    private val saving = mutableSetOf<String>()

    /** Counts saves that landed, so a load knows which ones its report includes. */
    private var landed = 0

    /** Bumped by the lock wipe: an answer from before it is dropped. */
    private var epoch = 0

    private class Saved(val kinds: Set<HealthKind>, val seq: Int)

    init {
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> wipe()
                    VaultEvent.ItemsChanged -> load()
                    else -> Unit
                }
            }
        }
    }

    fun shown() = load()

    fun filter(f: HealthFilter) {
        _state.update { it.copy(filter = f, rows = rows(f)) }
    }

    fun dismiss(id: String, kind: HealthKind) = change(id) { it + kind }

    fun undo(id: String, kind: HealthKind) = change(id) { it - kind }

    /** Rust's link for this login and check, unchanged, or null when it has none. */
    suspend fun helpUrl(id: String, kind: HealthKind): String? =
        (vault.healthHelpUrl(id, kind) as? Outcome.Ok)?.value

    private fun change(id: String, edit: (Set<HealthKind>) -> Set<HealthKind>) {
        // What was last saved for this login, else the report's dismissed checks.
        val dismissed = saved[id]?.kinds
            ?: issues.filter { it.itemId == id && it.dismissed }.flatMap { it.kinds }.toSet()
        val next = edit(dismissed).sortedBy { it.ordinal }
        saving += id
        // Recorded at once, so a second tap before Rust answers builds on this one.
        val before = saved[id]
        saved[id] = Saved(next.toSet(), Int.MAX_VALUE)
        _state.update { it.copy(rows = rows(it.filter)) }
        val asked = epoch
        viewModelScope.launch {
            val r = vault.setHealthIgnored(id, next)
            if (asked != epoch) return@launch
            saving -= id
            when (r) {
                is Outcome.Failed -> {
                    if (before == null) saved -= id else saved[id] = before
                    _state.update { it.copy(errorCode = r.code, rows = rows(it.filter)) }
                }
                is Outcome.Ok -> {
                    landed += 1
                    saved[id] = Saved(next.toSet(), landed)
                    // The row is free now, even if the reload below fails.
                    _state.update { it.copy(rows = rows(it.filter)) }
                    load()
                }
            }
        }
    }

    private fun load() {
        pending?.cancel()
        val includes = landed
        pending = viewModelScope.launch {
            val all = vault.list()
            val report = vault.health()
            if (report is Outcome.Failed) {
                _state.update { it.copy(loading = false, errorCode = report.code) }
                return@launch
            }
            val view = (report as Outcome.Ok).value
            titles = (all as? Outcome.Ok)?.value.orEmpty().associate { it.id to (it.title to it.subtitle) }
            issues = view.issues
            saved.entries.removeAll { it.value.seq <= includes }
            _state.update {
                it.copy(
                    counts = view.counts,
                    total = view.counts.total(),
                    rows = rows(it.filter),
                    loading = false,
                    errorCode = null,
                )
            }
        }
    }

    /** An issue's kinds as shown: what was dismissed or restored since the report is hidden until the next one. */
    private fun visibleKinds(issue: HealthIssueView): List<HealthKind> {
        val ignored = saved[issue.itemId]?.kinds ?: return issue.kinds
        return issue.kinds.filter { (it in ignored) == issue.dismissed }
    }

    private fun rows(f: HealthFilter): List<HealthRow> {
        val shown = issues.map { it to visibleKinds(it) }.filter { (_, kinds) -> kinds.isNotEmpty() }
        val active = shown.filter { (issue, _) -> !issue.dismissed }
        return shown
            .filter { (issue, kinds) ->
                when (f) {
                    HealthFilter.All -> !issue.dismissed
                    HealthFilter.Dismissed -> issue.dismissed
                    is HealthFilter.Kind -> !issue.dismissed && f.kind in kinds
                }
            }
            .mapNotNull { (issue, kinds) ->
                val (title, subtitle) = titles[issue.itemId] ?: return@mapNotNull null
                HealthRow(
                    id = issue.itemId,
                    title = title,
                    subtitle = subtitle,
                    kinds = kinds,
                    groupSize = issue.reusedGroup?.let { g ->
                        reusedIn(active.count { (o, k) -> o.reusedGroup == g && HealthKind.REUSED in k })
                    },
                    duplicates = issue.duplicateGroup?.let { g ->
                        duplicatesOf(active.count { (o, k) -> o.duplicateGroup == g && HealthKind.DUPLICATE in k })
                    },
                    dismissed = issue.dismissed,
                    // Only while the save is in flight: once it landed, the next change builds on it.
                    busy = issue.itemId in saving,
                )
            }
    }

    private fun wipe() {
        epoch += 1
        pending?.cancel()
        issues = emptyList()
        titles = emptyMap()
        saved.clear()
        saving.clear()
        _state.value = HealthUiState()
    }
}
