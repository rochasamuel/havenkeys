package net.havenkeys.android.ui.search

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldDecorator
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.KitTextInput
import net.havenkeys.android.ui.shell.searchPill
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The pill as a field: the search glyph, the query, and a clear button once
 * something is typed. TalkBack reads the placeholder "Search HavenKeys" as the
 * field's text; there is no contentDescription, which would hide what is typed.
 * The keyboard is asked not to learn or correct what is typed.
 */
@Composable
internal fun SearchField(state: TextFieldState, modifier: Modifier = Modifier) {
    val colors = HavenTheme.colors
    val label = stringResource(R.string.shell_search)
    val keyboard = LocalSoftwareKeyboardController.current
    KitTextInput {
        BasicTextField(
            state = state,
            modifier = modifier,
            textStyle = HavenTheme.type.value.copy(color = colors.textStrong),
            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Search),
            onKeyboardAction = { keyboard?.hide() },
            lineLimits = TextFieldLineLimits.SingleLine,
            cursorBrush = SolidColor(colors.brass),
            decorator = TextFieldDecorator { field ->
                Row(
                    Modifier
                        .heightIn(min = HavenSpacing.touch)
                        .clip(HavenShape.pill)
                        .background(colors.searchPill)
                        .padding(start = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    IconGlyph(HavenIcon.Search, contentDescription = null, tint = colors.muted, size = 18.dp)
                    Spacer(Modifier.width(8.dp))
                    Box(Modifier.weight(1f).padding(vertical = 12.dp)) {
                        if (state.text.isEmpty()) {
                            // Not cleared: it merges into the field's node, so TalkBack reads it with the text.
                            HavenText(label, style = HavenTheme.type.value, color = colors.muted)
                        }
                        field()
                    }
                    if (state.text.isNotEmpty()) {
                        HavenIconButton(
                            HavenIcon.X,
                            stringResource(R.string.search_clear_field),
                            onClick = { state.clearText() },
                        )
                    } else {
                        Spacer(Modifier.width(12.dp))
                    }
                }
            },
        )
    }
}
