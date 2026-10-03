package net.havenkeys.android.credentials

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.ChoiceRow
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenSheet
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.ProgressRing
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * "Save a passkey to HavenKeys?" as a sheet over the site asking. Closing it
 * (drag, backdrop, Back) answers as its own button would: Close when the
 * passkey already exists, Cancel otherwise. Taller than the window allows,
 * it scrolls (many logins, a large font).
 */
@Composable
fun PasskeyCreateScreen(
    state: PasskeyCreateUiState,
    onSelect: (String?) -> Unit,
    onSave: () -> Unit,
    onCancel: () -> Unit,
    onClose: () -> Unit,
) {
    // While the passkey is being created, drag, backdrop and Back do nothing: the site is not answered twice.
    HavenSheet(
        onDismiss = { if (!state.busy) (if (state.excluded) onClose else onCancel)() },
        title = stringResource(R.string.passkey_save_title),
        dismissible = !state.busy,
    ) {
        PasskeyCreateContent(state, onSelect, onSave, onCancel, onClose)
    }
}

/** The sheet's content, without its window: the screenshots draw it inline. */
@Composable
internal fun ColumnScope.PasskeyCreateContent(
    state: PasskeyCreateUiState,
    onSelect: (String?) -> Unit,
    onSave: () -> Unit,
    onCancel: () -> Unit,
    onClose: () -> Unit,
) {
    when {
        state.loading -> ProgressRing(
            progress = null,
            Modifier.align(Alignment.CenterHorizontally).padding(vertical = 24.dp),
        )
        state.excluded -> {
            HavenText(
                stringResource(R.string.passkey_exists),
                style = HavenTheme.type.value,
                color = HavenTheme.colors.textStrong,
            )
            HavenButton(
                stringResource(R.string.passkey_close),
                onClick = onClose,
                Modifier.fillMaxWidth().padding(top = 16.dp),
                style = ButtonStyle.Secondary,
            )
        }
        state.planFailed -> {
            state.error?.let { ErrorLine(it) }
            HavenButton(
                stringResource(R.string.passkey_cancel),
                onClick = onCancel,
                Modifier.fillMaxWidth().padding(top = 16.dp),
                style = ButtonStyle.Secondary,
            )
        }
        else -> Choice(state, onSelect, onSave, onCancel)
    }
}

@Composable
private fun Choice(state: PasskeyCreateUiState, onSelect: (String?) -> Unit, onSave: () -> Unit, onCancel: () -> Unit) {
    val colors = HavenTheme.colors
    HavenText(state.rpId, style = HavenTheme.type.rowTitle, color = colors.textStrong)
    HavenText(
        stringResource(
            R.string.passkey_account,
            state.userName.ifBlank { stringResource(R.string.passkey_no_account_name) },
        ),
        color = colors.muted,
    )
    SectionHeader(stringResource(R.string.passkey_save_to), Modifier.padding(top = 12.dp))
    InsetGroup(Modifier.selectableGroup()) {
        state.homes.forEach { home ->
            row {
                ChoiceRow(
                    home.title,
                    state.selected == home.id,
                    onClick = { onSelect(home.id) },
                    detail = home.username,
                )
            }
        }
        row {
            ChoiceRow(
                stringResource(R.string.passkey_new_login),
                state.selected == null,
                onClick = { onSelect(null) },
                detail = stringResource(R.string.passkey_new_login_detail),
            )
        }
    }
    state.error?.let { ErrorLine(it) }
    Row(Modifier.fillMaxWidth().padding(top = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        HavenButton(
            stringResource(R.string.passkey_cancel),
            onClick = onCancel,
            Modifier.weight(1f),
            style = ButtonStyle.Quiet,
        )
        HavenButton(stringResource(R.string.passkey_save), onClick = onSave, Modifier.weight(1f), enabled = !state.busy)
    }
}
