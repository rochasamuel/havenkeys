package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.SecureFlagPolicy
import net.havenkeys.android.ui.components.SecureDialogWindow
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * A question that needs an answer. Its window sets FLAG_SECURE, drops taps
 * through another app's overlay and is excluded from autofill. The first
 * tap on either button answers; a second tap does nothing.
 *
 * [content] sits between the message and the buttons (a field and its
 * error). The caller owns [confirmEnabled] and [busy]: while busy the
 * confirm button shows progress and nothing dismisses the dialog (Back, an
 * outside tap and Cancel do nothing). The dialog may answer again when the
 * answer failed and the dialog stayed open: [busy] going back to false, or
 * a changed [answerKey], re-arms it. [answerKey] is an attempt counter the
 * caller bumps on every failed confirm, never a message: two failures in a
 * row can show the same error, and only a changed key re-arms the dialog.
 */
@Composable
@Suppress("LongParameterList")
fun HavenDialog(
    title: String,
    onDismiss: () -> Unit,
    confirm: DialogAction,
    modifier: Modifier = Modifier,
    message: String? = null,
    dismiss: DialogAction? = null,
    content: (@Composable ColumnScope.() -> Unit)? = null,
    confirmEnabled: Boolean = true,
    busy: Boolean = false,
    answerKey: Any? = null,
) {
    val answer = rememberDialogAnswer(busy, answerKey)
    Dialog(
        onDismissRequest = { if (!busy) answer.run(onDismiss) },
        properties = DialogProperties(securePolicy = SecureFlagPolicy.SecureOn),
    ) {
        SecureDialogWindow(ignoreObscuredTouches = true)
        WindowDim(HavenTheme.colors.scrim.alpha)
        DialogContent(title, confirm, modifier, message, dismiss, content, confirmEnabled, busy, answer)
    }
}

/** A dialog's button: its label, what it does, and whether it destroys something. */
class DialogAction(val label: String, val onClick: () -> Unit, val danger: Boolean = false)

/** The dialog as it draws, without its window: the catalogue shows it inline. */
@Composable
@Suppress("LongParameterList")
internal fun DialogSurface(
    title: String,
    confirm: DialogAction,
    modifier: Modifier = Modifier,
    message: String? = null,
    dismiss: DialogAction? = null,
    content: (@Composable ColumnScope.() -> Unit)? = null,
    confirmEnabled: Boolean = true,
    busy: Boolean = false,
    answerKey: Any? = null,
) {
    val answer = rememberDialogAnswer(busy, answerKey)
    DialogContent(title, confirm, modifier, message, dismiss, content, confirmEnabled, busy, answer)
}

@Composable
private fun rememberDialogAnswer(busy: Boolean, answerKey: Any?): DialogAnswer {
    // The keys re-arm the guard: a failed confirm ends busy (or changes the key) and the dialog stays open.
    var answered by remember(busy, answerKey) { mutableStateOf(false) }
    return DialogAnswer(answered) { action ->
        if (!answered) {
            answered = true
            action()
        }
    }
}

@Suppress("LongParameterList")
@Composable
private fun DialogContent(
    title: String,
    confirm: DialogAction,
    modifier: Modifier,
    message: String?,
    dismiss: DialogAction?,
    content: (@Composable ColumnScope.() -> Unit)?,
    confirmEnabled: Boolean,
    busy: Boolean,
    answer: DialogAnswer,
) {
    val colors = HavenTheme.colors
    fun once(action: () -> Unit): () -> Unit = { answer.run(action) }
    val answered = answer.done
    Column(
        modifier
            .widthIn(max = 360.dp)
            .fillMaxWidth()
            .shadow(24.dp, HavenShape.dialog)
            .clip(HavenShape.dialog)
            .background(colors.raised)
            .padding(24.dp)
            .semantics { paneTitle = title },
    ) {
        HavenText(title, Modifier.semantics { heading() }, style = HavenTheme.type.title, color = colors.textStrong)
        if (message != null) {
            HavenText(message, Modifier.padding(top = 8.dp), style = HavenTheme.type.body, color = colors.text)
        }
        if (content != null) {
            Column(Modifier.fillMaxWidth().padding(top = 16.dp)) { content() }
        }
        Row(
            Modifier.align(Alignment.End).padding(top = 24.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (dismiss != null) {
                HavenButton(
                    dismiss.label,
                    once(dismiss.onClick),
                    style = ButtonStyle.Quiet,
                    enabled = !answered && !busy,
                )
            }
            if (busy) ProgressRing(progress = null, size = 20.dp)
            HavenButton(
                confirm.label,
                once(confirm.onClick),
                style = if (confirm.danger) ButtonStyle.Danger else ButtonStyle.Primary,
                enabled = !answered && !busy && confirmEnabled,
            )
        }
    }
}

/** Whether a button has answered yet, and the guard that lets only the first answer through. */
private class DialogAnswer(val done: Boolean, val run: (() -> Unit) -> Unit)

@PreviewLightDark
@Composable
private fun HavenDialogPreview() {
    KitPreview {
        DialogSurface(
            title = "Remove this device?",
            confirm = DialogAction("Remove", {}, danger = true),
            message = "Its local copy of the vault is erased.",
            dismiss = DialogAction("Cancel", {}),
        )
    }
}
