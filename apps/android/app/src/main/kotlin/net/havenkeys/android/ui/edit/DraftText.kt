package net.havenkeys.android.ui.edit

import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.snapshotFlow
import kotlinx.coroutines.flow.drop

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
    val latest by rememberUpdatedState(onEdit)
    LaunchedEffect(state) {
        snapshotFlow { state.text.toString() }.drop(1).collect { latest(it) }
    }
    return state
}
