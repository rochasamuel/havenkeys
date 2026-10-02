package net.havenkeys.android.autofill

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.SecureFlagPolicy
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.SecureDialogWindow

/**
 * The question a gated card or identity row asks before Rust is asked for
 * anything. In a secure window that ignores taps through overlays. Each
 * button answers once.
 */
@Composable
@Suppress("LongParameterList")
internal fun WalletConfirmDialog(
    question: String,
    detail: String?,
    confirmLabel: String,
    alternativeLabel: String?,
    onConfirm: () -> Unit,
    onAlternative: () -> Unit,
    onDismiss: () -> Unit,
) {
    var answered by remember { mutableStateOf(false) }
    fun once(action: () -> Unit): () -> Unit = {
        if (!answered) {
            answered = true
            action()
        }
    }
    AlertDialog(
        onDismissRequest = once(onDismiss),
        // FLAG_SECURE stated, not inherited from the activity's window.
        properties = DialogProperties(securePolicy = SecureFlagPolicy.SecureOn),
        text = {
            Column {
                SecureDialogWindow(ignoreObscuredTouches = true)
                Text(question)
                detail?.let {
                    Text(
                        it,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
            }
        },
        confirmButton = {
            Row {
                alternativeLabel?.let {
                    TextButton(enabled = !answered, onClick = once(onAlternative)) { Text(it) }
                }
                TextButton(enabled = !answered, onClick = once(onConfirm)) { Text(confirmLabel) }
            }
        },
        dismissButton = {
            TextButton(enabled = !answered, onClick = once(onDismiss)) {
                Text(stringResource(R.string.autofill_cancel))
            }
        },
    )
}
