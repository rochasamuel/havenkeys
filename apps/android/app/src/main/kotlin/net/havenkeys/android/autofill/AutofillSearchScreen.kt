package net.havenkeys.android.autofill

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.HavenDialog
import net.havenkeys.android.ui.kit.ItemRow
import net.havenkeys.android.ui.kit.RowLeading
import net.havenkeys.android.ui.search.SearchField
import net.havenkeys.android.ui.shell.EmptyLine
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.insetGroup
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.AutofillMatch

private const val SEARCH_DEBOUNCE_MS = 200L

/** The app being filled: [packageName] identifies it; [label] is the app's own choice. */
internal data class CallerApp(val packageName: String, val label: String?)

/**
 * "Search HavenKeys…" (Android spec §7.2): the user finds a login for an app
 * nothing binds yet, then confirms "Use <login> in <app>?". [onConfirmed]
 * answers an error code, or null once it has filled. The query is
 * `remember`ed only, never saved.
 */
@Composable
internal fun AutofillSearchScreen(
    search: suspend (String) -> Outcome<List<AutofillMatch>>,
    app: CallerApp,
    onConfirmed: suspend (AutofillMatch) -> String?,
    modifier: Modifier = Modifier,
) {
    val query = remember { TextFieldState() }
    val found = rememberSearch(query, search)
    var picked by remember { mutableStateOf<AutofillMatch?>(null) }
    var errorCode by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val focus = remember { FocusRequester() }
    val gutter = Modifier.padding(horizontal = HavenSpacing.gutter)
    LaunchedEffect(focus) { focus.requestFocus() }

    Column(modifier.fillMaxSize().background(HavenTheme.colors.pane).safeDrawingPadding().imePadding()) {
        SearchField(query, gutter.padding(vertical = 8.dp).fillMaxWidth().focusRequester(focus))
        errorCode?.let { ErrorLine(it, gutter) }
        // A failed search says why rather than looking like an empty result.
        found.failed?.let { ErrorLine(it, gutter) }
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = HavenSpacing.gutter)) {
            if (query.text.isNotBlank() && found.answeredEmpty) {
                item(key = "none") { EmptyLine(stringResource(R.string.vault_no_matches), gutter) }
            }
            insetGroup(found.results, key = { it.id }) { match ->
                ItemRow(
                    match.title,
                    match.username,
                    RowLeading.Monogram(match.title),
                    onClick = { if (!busy) picked = match },
                    hasCode = match.hasTotp,
                )
            }
        }
    }

    picked?.let { match ->
        UseInAppDialog(
            match,
            app,
            busy = busy,
            onCancel = { picked = null },
            onUse = {
                busy = true
                scope.launch {
                    errorCode = onConfirmed(match)
                    busy = false
                    picked = null
                }
            },
        )
    }
}

/** The search's answer for the typed query; "searching" covers the debounce and Rust's answer. */
private class SearchAnswer {
    var results by mutableStateOf(emptyList<AutofillMatch>())
    var searching by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)

    /** The error once the latest search has answered with one. */
    val failed: String? get() = error.takeUnless { searching }

    /** The latest search answered, without an error, with nothing: only then is it "No matches". */
    val answeredEmpty: Boolean get() = !searching && error == null && results.isEmpty()

    fun reset() {
        results = emptyList()
        error = null
        searching = false
    }

    fun answer(outcome: Outcome<List<AutofillMatch>>) {
        when (outcome) {
            is Outcome.Ok -> {
                results = outcome.value
                error = null
            }
            is Outcome.Failed -> {
                results = emptyList()
                error = outcome.code
            }
        }
        searching = false
    }
}

/** Searches [query] after a short pause in typing; a newer query cancels the older search. */
@Composable
private fun rememberSearch(
    query: TextFieldState,
    search: suspend (String) -> Outcome<List<AutofillMatch>>,
): SearchAnswer {
    val answer = remember { SearchAnswer() }
    LaunchedEffect(query) {
        snapshotFlow { query.text.toString() }.collectLatest { typed ->
            if (typed.isBlank()) {
                answer.reset()
            } else {
                answer.searching = true
                delay(SEARCH_DEBOUNCE_MS)
                answer.answer(search(typed.trim()))
            }
        }
    }
    return answer
}

@Composable
private fun UseInAppDialog(
    match: AutofillMatch,
    app: CallerApp,
    busy: Boolean,
    onCancel: () -> Unit,
    onUse: () -> Unit,
) {
    // The package name is what identifies the app (spec §7.2); the label is
    // the app's own choice and could name anything, even this login.
    HavenDialog(
        title = stringResource(R.string.autofill_use_in_app, match.title, app.packageName),
        onDismiss = onCancel,
        confirm = DialogAction(stringResource(R.string.autofill_use), onUse),
        message = app.label?.takeIf { it.isNotBlank() && it != app.packageName }
            ?.let { stringResource(R.string.autofill_app_label, it) },
        dismiss = DialogAction(stringResource(R.string.autofill_cancel), onCancel),
        busy = busy,
    )
}
