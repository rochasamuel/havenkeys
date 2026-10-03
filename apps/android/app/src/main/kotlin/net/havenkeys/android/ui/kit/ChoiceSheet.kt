package net.havenkeys.android.ui.kit

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * A sheet of single choices: each value a radio button for TalkBack, a brass
 * check on the current one. A pick closes the sheet at once, then saves the
 * value if it changed (the pick is the motion; the sheet does not linger).
 */
@Suppress("LongParameterList") // A sheet's title, its values, the current one, how to say each, and two outcomes.
@Composable
fun <T> ChoiceSheet(
    title: String,
    values: List<T>,
    selected: T,
    label: @Composable (T) -> String,
    onSelect: (T) -> Unit,
    onDismiss: () -> Unit,
) {
    HavenSheet(onDismiss = onDismiss, title = title) {
        InsetGroup(Modifier.selectableGroup()) {
            values.forEach { value ->
                row {
                    ChoiceRow(
                        label(value),
                        selected = value == selected,
                        onClick = {
                            onDismiss()
                            if (value != selected) onSelect(value)
                        },
                    )
                }
            }
        }
    }
}

/** One choice in an [InsetGroup] marked `selectableGroup()`: a radio button, a brass check when chosen. */
@Composable
fun ChoiceRow(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    detail: String? = null,
) {
    val colors = HavenTheme.colors
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.rowMin)
            .selectable(
                selected = selected,
                interactionSource = null,
                indication = HavenPress,
                role = Role.RadioButton,
                onClick = onClick,
            )
            .padding(horizontal = HavenSpacing.rowX, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) { GroupRowText(label, detail) }
        if (selected) IconGlyph(HavenIcon.Check, contentDescription = null, tint = colors.brass, size = 20.dp)
    }
}

@PreviewLightDark
@Composable
private fun ChoiceRowPreview() {
    KitPreview {
        InsetGroup(Modifier.selectableGroup()) {
            row { ChoiceRow("After 5 minutes", selected = true, onClick = {}) }
            row { ChoiceRow("New login", selected = false, onClick = {}, detail = "With this passkey only") }
        }
    }
}
