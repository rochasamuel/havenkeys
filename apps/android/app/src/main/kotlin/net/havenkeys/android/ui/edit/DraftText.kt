package net.havenkeys.android.ui.edit

import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.dropWhile

/**
 * A kit field's text over one draft value. `remember`ed, never saved (spec
 * §9.4: never `rememberTextFieldState`, which writes its text into the saved
 * instance state). It starts from [initial] and starts again when a key
 * changes: a new draft, or a value that just arrived from Rust. Every edit
 * after that goes to [onEdit]; the starting value is not an edit.
 */
@Composable
internal fun rememberDraftText(vararg keys: Any?, initial: () -> String, onEdit: (String) -> Unit): TextFieldState {
    val state = remember(*keys) { TextFieldState(initial()) }
    // The value the state started from, read in the same composition that made it. An edit made before
    // the collector starts is the first value seen, so skipping "the first value" would lose it; only the
    // start is skipped, and only until something else is seen (a later return to it is an edit).
    val start = remember(state) { state.text.toString() }
    val latest by rememberUpdatedState(onEdit)
    LaunchedEffect(state) {
        editsAfter(start, snapshotFlow { state.text.toString() }).collect { latest(it) }
    }
    return state
}

/** The values of [texts] that are edits: everything from the first value that is not [start]. */
internal fun editsAfter(start: String, texts: Flow<String>): Flow<String> = texts.dropWhile { it == start }
