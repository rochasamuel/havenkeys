package net.havenkeys.android.autofill

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.HavenDialog

/**
 * Opened by a tapped look-alike warning row (spec 2026-10-13-lookalike-
 * warning §6). Explains; fills nothing. HavenDialog answers once, sets
 * FLAG_SECURE, drops taps through overlays and keeps its window from autofill.
 */
@Composable
internal fun LookalikeDialog(site: String, onDismiss: () -> Unit) {
    HavenDialog(
        title = stringResource(R.string.autofill_lookalike_dialog_title, site),
        onDismiss = onDismiss,
        confirm = DialogAction(stringResource(R.string.autofill_lookalike_dialog_close), onDismiss),
        message = stringResource(R.string.autofill_lookalike_dialog_body, site),
    )
}
