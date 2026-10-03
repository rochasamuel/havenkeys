package net.havenkeys.android.autofill

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.HavenDialog

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
    // HavenDialog answers once, sets FLAG_SECURE on its window, drops taps
    // through overlays and keeps the window from autofill.
    HavenDialog(
        title = question,
        onDismiss = onDismiss,
        confirm = DialogAction(confirmLabel, onConfirm),
        message = detail,
        dismiss = DialogAction(stringResource(R.string.autofill_cancel), onDismiss),
        alternative = alternativeLabel?.let { DialogAction(it, onAlternative) },
    )
}
