package net.havenkeys.android.ui.item

import android.content.res.Resources
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleStartEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.ScreenBar
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.health.HealthChips
import net.havenkeys.android.ui.kit.CopyButton
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowField
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenMenu
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.ItemTile
import net.havenkeys.android.ui.kit.MenuItem
import net.havenkeys.android.ui.kit.Pill
import net.havenkeys.android.ui.kit.PillTone
import net.havenkeys.android.ui.kit.ToastState
import net.havenkeys.android.ui.kit.ToastTone
import net.havenkeys.android.ui.kit.rememberToastState
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.leading
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.ViewField

/** The header's tile: larger than a row's, so the row's tile grows into it. */
private val HeaderTile = 56.dp

/**
 * One item: its overview from the ViewModel, and each hidden field read from
 * Rust only when the user taps reveal or copy. A revealed value or a code
 * lives in this composition only (spec §9.4). [titleModifier] and
 * [tileModifier] carry the tapped row's title and tile into the header.
 */
@Composable
fun ItemScreen(
    viewModel: ItemViewModel,
    clipboard: SensitiveClipboard,
    online: Boolean,
    navigation: ItemNavigation,
    modifier: Modifier = Modifier,
    titleModifier: Modifier = Modifier,
    tileModifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val toasts = rememberToastState()
    val scope = rememberCoroutineScope()
    val resources = LocalResources.current
    val actions = remember(viewModel, clipboard, toasts, scope, resources) {
        FieldActions(viewModel, clipboard, toasts, scope, resources)
    }

    HavenScaffold(
        modifier = modifier,
        topBar = {
            ScreenBar(onBack = navigation.onBack, online = online, onLock = navigation.onLock) {
                ItemActions(state.view, online, navigation.onEdit, onDelete = { actions.trash(navigation) })
            }
        },
        toastState = toasts,
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = HavenSpacing.gutter)
                .padding(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            state.view?.let { view ->
                ItemHeader(view.summary, titleModifier, tileModifier)
                ItemTags(view.summary.tags)
                if (state.health.isNotEmpty()) {
                    // Vault health's checks for this login, under its title, before its fields.
                    HealthChips(state.health, state.reusedIn, state.duplicates, Modifier.padding(bottom = 16.dp))
                }
                ItemFields(view, actions)
            }
            state.errorCode?.let { ErrorLine(it) }
        }
    }
}

@Composable
private fun ItemHeader(summary: ItemSummary, titleModifier: Modifier, tileModifier: Modifier) {
    Row(Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 20.dp), verticalAlignment = Alignment.CenterVertically) {
        ItemTile(summary.leading(), tileModifier, size = HeaderTile)
        Spacer(Modifier.width(14.dp))
        HavenText(
            summary.title,
            Modifier.weight(1f).then(titleModifier).semantics { heading() },
            style = HavenTheme.type.headline,
            color = HavenTheme.colors.textStrong,
        )
    }
}

/** The item's tags as markers under its title: brass pills with the tag glyph, with no action. */
@Composable
private fun ItemTags(tags: List<String>) {
    if (tags.isEmpty()) return
    val label = stringResource(R.string.item_tags)
    FlowRow(
        Modifier.padding(bottom = 16.dp).semantics { contentDescription = label },
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        // Brass, with the tag glyph: the vault-health chips under them stay outlined.
        tags.forEach { Pill(it, tone = PillTone.Brass, icon = HavenIcon.Tag) }
    }
}

/**
 * The fields as one group. Each reveal is keyed by its field inside its row,
 * so a reload that moves a field starts that row masked rather than showing
 * a value on the wrong row.
 */
@Composable
private fun ItemFields(view: ItemView, actions: FieldActions) {
    if (view.fields.isEmpty()) return
    InsetGroup {
        view.fields.forEach { field -> row { key(field.key) { FieldRow(field, actions) } } }
    }
}

/** Edit, and Delete (to the Trash) behind More; both write to the server, so both need it online. */
@Composable
private fun ItemActions(view: ItemView?, online: Boolean, onEdit: () -> Unit, onDelete: () -> Unit) {
    HavenIconButton(
        HavenIcon.Edit,
        stringResource(R.string.item_edit),
        onClick = onEdit,
        enabled = online && view != null,
    )
    // The identity is never deleted from the phone.
    if (view == null || view.summary.kind == ItemKind.IDENTITY) return
    var open by remember { mutableStateOf(false) }
    // Going offline while More is open closes it (its only entry waits for the server), and it
    // stays closed when the server comes back.
    LaunchedEffect(online) { if (!online) open = false }
    Box {
        // Delete is More's only entry, so More itself waits for the server.
        HavenIconButton(
            HavenIcon.More,
            stringResource(R.string.vault_more),
            onClick = { open = true },
            enabled = online,
        )
        HavenMenu(
            expanded = open && online,
            onDismiss = { open = false },
            items = listOf(MenuItem(stringResource(R.string.item_delete), onDelete, HavenIcon.Trash, danger = true)),
        )
    }
}

/** What a field (and the item) can do; every value passes through here without being kept. */
private class FieldActions(
    val viewModel: ItemViewModel,
    private val clipboard: SensitiveClipboard,
    private val toasts: ToastState,
    private val scope: CoroutineScope,
    private val resources: Resources,
) {
    fun reveal(key: String, into: RevealState) {
        if (into.value != null) {
            into.clear()
            return
        }
        // A clear while Rust answers (the app stopped, the row left) wins: the late answer is dropped.
        val asked = into.generation
        scope.launch {
            when (val r = viewModel.reveal(key)) {
                is Outcome.Ok -> into.showIfCurrent(asked, r.value)
                is Outcome.Failed -> fail(r.code)
            }
        }
    }

    /** Asks Rust for one plain field and hands it to [show]; a stop cancels the answer. */
    fun load(key: String, show: (String) -> Unit): Job = scope.launch {
        when (val r = viewModel.reveal(key)) {
            is Outcome.Ok -> show(r.value)
            is Outcome.Failed -> fail(r.code)
        }
    }

    fun copyField(key: String, label: String) {
        scope.launch {
            when (val r = viewModel.reveal(key)) {
                is Outcome.Ok -> copy(label, r.value)
                is Outcome.Failed -> fail(r.code)
            }
        }
    }

    /**
     * Delete, with no question: the item goes to the Trash and the screen
     * underneath offers Undo. The title is read before Rust answers, since
     * the overview reloads without the item once it has gone.
     */
    fun trash(navigation: ItemNavigation) {
        val title = viewModel.state.value.view?.summary?.title ?: return
        // A second Delete while the first is with the server would only fail.
        if (trashing) return
        trashing = true
        scope.launch {
            when (val r = viewModel.trash()) {
                is Outcome.Ok -> if (r.value) navigation.onTrashed(title) else navigation.onDeleted(title)
                is Outcome.Failed -> {
                    trashing = false
                    fail(r.code)
                }
            }
        }
    }

    private var trashing = false

    fun copyShown(label: String, value: String) {
        scope.launch { copy(label, value) }
    }

    private suspend fun copy(label: String, value: String) {
        val seconds = viewModel.clipboardClearSeconds()
        clipboard.copy(label, value, seconds)
        viewModel.copied()
        toasts.show(resources.getString(R.string.copied, label, seconds))
    }

    private fun fail(code: String) = toasts.show(resources.getString(errorText(code)), ToastTone.Alert)
}

@Composable
private fun FieldRow(field: ViewField, actions: FieldActions) {
    val label = stringResource(fieldLabel(field.label))
    val shown = field.value
    when {
        field.kind == FieldKind.TOTP -> CodeField(label, actions)
        shown != null -> ShownRow(label, shown, onCopy = { actions.copyShown(label, shown) })
        field.kind == FieldKind.TEXT -> OpenRow(field.key, label, actions)
        else -> {
            val reveal = rememberRevealState()
            SecretRow(
                label,
                reveal.value,
                onReveal = { actions.reveal(field.key, reveal) },
                onCopy = { actions.copyField(field.key, label) },
            )
        }
    }
}

/**
 * A plain field whose value Rust gives one at a time (an identity's name, a
 * card's holder, a secure note's body): asked for as the row appears and
 * shown as on the desktop, with no eye. Like a revealed value it lives only
 * in this composition, and it is dropped when the app stops or the row leaves.
 */
@Composable
private fun OpenRow(key: String, label: String, actions: FieldActions) {
    var value by remember(key) { mutableStateOf<String?>(null) }
    LifecycleStartEffect(key, actions) {
        val job = actions.load(key) { value = it }
        onStopOrDispose {
            job.cancel()
            value = null
        }
    }
    GroupRow(trailing = { CopyButton(label, onCopy = { actions.copyField(key, label) }) }) {
        GroupRowField(label, value.orEmpty())
    }
}

/** The live code, asked of Rust once a second only while the screen is visible. */
@Composable
private fun CodeField(label: String, actions: FieldActions) {
    val ticks = remember(actions) { actions.viewModel.totpTicks() }
    val now by ticks.collectAsStateWithLifecycle(initialValue = null)
    CodeRow(
        label,
        (now as? Outcome.Ok)?.value,
        failed = now is Outcome.Failed,
        onCopy = { code -> actions.copyShown(label, code) },
    )
}

private val labels = mapOf(
    "username" to R.string.field_username,
    "password" to R.string.field_password,
    "totp" to R.string.field_totp,
    "website" to R.string.field_website,
    "notes" to R.string.field_notes,
    "content" to R.string.field_content,
    "card.holder" to R.string.field_card_holder,
    "card.number" to R.string.field_card_number,
    "card.code" to R.string.field_card_code,
    "card.expiry" to R.string.field_card_expiry,
    "card.notes" to R.string.field_card_notes,
    "identity.first_name" to R.string.field_identity_first_name,
    "identity.middle_name" to R.string.field_identity_middle_name,
    "identity.last_name" to R.string.field_identity_last_name,
    "identity.gender" to R.string.field_identity_gender,
    "identity.birth_date" to R.string.field_identity_birth_date,
    "identity.occupation" to R.string.field_identity_occupation,
    "identity.company" to R.string.field_identity_company,
    "identity.job_title" to R.string.field_identity_job_title,
    "identity.cpf" to R.string.field_identity_cpf,
    "identity.rg" to R.string.field_identity_rg,
    "identity.passport" to R.string.field_identity_passport,
    "identity.drivers_license" to R.string.field_identity_drivers_license,
    "identity.email" to R.string.field_identity_email,
    "identity.mobile_phone" to R.string.field_identity_mobile_phone,
    "identity.home_phone" to R.string.field_identity_home_phone,
    "identity.work_phone" to R.string.field_identity_work_phone,
    "identity.street" to R.string.field_identity_street,
    "identity.number" to R.string.field_identity_number,
    "identity.complement" to R.string.field_identity_complement,
    "identity.neighborhood" to R.string.field_identity_neighborhood,
    "identity.city" to R.string.field_identity_city,
    "identity.state" to R.string.field_identity_state,
    "identity.postal_code" to R.string.field_identity_postal_code,
    "identity.country" to R.string.field_identity_country,
    "identity.username" to R.string.field_identity_username,
    "identity.website" to R.string.field_identity_website,
    "identity.notes" to R.string.field_identity_notes,
)

/** The text for a `ViewField.label` key; a custom field (empty label) reads "Field". */
@StringRes
internal fun fieldLabel(label: String): Int = labels[label] ?: R.string.field_custom
