package net.havenkeys.android.ui.item

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.MaskedValue
import net.havenkeys.android.ui.components.RevealedValue
import net.havenkeys.android.ui.kit.CopyButton
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowField
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.ProgressRing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.TotpNow

/** The ring turns ember for the last seconds of a code. */
private const val ENDING_SECONDS = 5

private const val GROUP_MIN = 6

/** A value Rust gave with the overview (a username, a website, a note): shown, with Copy. */
@Composable
internal fun ShownRow(label: String, value: String, onCopy: () -> Unit) {
    GroupRow(trailing = { CopyButton(label, onCopy) }) { GroupRowField(label, value) }
}

/**
 * A hidden field: the fixed mask until the eye reveals it, then the value in
 * mono. The caller's RevealState holds [revealed] and clears it (30 s, leave,
 * background, lock); this row keeps nothing.
 */
@Composable
internal fun SecretRow(label: String, revealed: String?, onReveal: () -> Unit, onCopy: () -> Unit) {
    GroupRow(
        trailing = {
            HavenIconButton(
                if (revealed == null) HavenIcon.Eye else HavenIcon.EyeOff,
                stringResource(if (revealed == null) R.string.reveal else R.string.hide, label),
                onClick = onReveal,
            )
            CopyButton(label, onCopy)
        },
    ) {
        HavenText(label, style = HavenTheme.type.label, color = HavenTheme.colors.muted)
        if (revealed == null) MaskedValue(label) else RevealedValue(revealed)
    }
}

/** The live code in two halves, the time it has left as a ring, and Copy (the whole code). */
@Composable
internal fun CodeRow(label: String, now: TotpNow?, failed: Boolean, onCopy: (String) -> Unit) {
    val colors = HavenTheme.colors
    val copy: (@Composable RowScope.() -> Unit)? = if (now == null) {
        null
    } else {
        { CopyButton(label, onCopy = { onCopy(now.code) }) }
    }
    GroupRow(trailing = copy) {
        HavenText(label, style = HavenTheme.type.label, color = colors.muted)
        when {
            now != null -> Row(
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                HavenText(groupedCode(now.code), style = HavenTheme.type.code, color = colors.textStrong)
                CodeRing(now.secondsRemaining.toInt(), now.period.toInt())
            }
            failed -> HavenText(stringResource(R.string.item_code_failed), color = colors.danger)
        }
    }
}

/** How long the code has left: drains once a second; ember in the last five. */
@Composable
private fun CodeRing(secondsRemaining: Int, period: Int, modifier: Modifier = Modifier) {
    ProgressRing(
        progress = if (period > 0) secondsRemaining.toFloat() / period else 0f,
        modifier = modifier,
        size = 24.dp,
        warn = secondsRemaining <= ENDING_SECONDS,
        contentDescription = pluralStringResource(
            R.plurals.item_seconds_remaining,
            secondsRemaining,
            secondsRemaining,
        ),
    )
}

/** "381492" → "381 492", "12345678" → "1234 5678": easier to read and type. */
internal fun groupedCode(code: String): String =
    if (code.length < GROUP_MIN) code else code.substring(0, code.length / 2) + " " + code.substring(code.length / 2)
