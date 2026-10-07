package net.havenkeys.android.ui.edit

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.KeyboardActionHandler
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.theme.HavenTheme

/** How many of the vault's tags are offered under the field as the user types. */
private const val SUGGESTIONS = 5

/**
 * An item's tags: each with Remove, then a field that adds what is typed
 * (Done, a comma, or leaving the field), with the vault's matching tags
 * under it to tap. At 20 tags the field gives way to a line saying so.
 */
@Composable
internal fun Tags(editor: EditorState, vaultTags: List<String>) {
    // `remember`ed, never saved state, like every draft field (spec §9.4).
    val text = remember(editor) { TextFieldState() }
    val typed = text.text.toString()
    val query = typed.trim().lowercase()
    val suggestions = if (query.isEmpty()) {
        emptyList()
    } else {
        vaultTags.filter { query in it && it !in editor.tags }.take(SUGGESTIONS)
    }
    LaunchedEffect(text) {
        // A comma ends a tag, as on the desktop: each one before it is added.
        snapshotFlow { text.text.toString() }.collect { now ->
            if (',' in now) {
                now.split(',').dropLast(1).forEach(editor::addTag)
                text.setTextAndPlaceCursorAtEnd(now.substringAfterLast(','))
            }
        }
    }
    val commit = { if (editor.addTag(text.text.toString())) text.clearText() }
    Column {
        SectionHeader(stringResource(R.string.edit_tags))
        InsetGroup {
            editor.tags.forEach { tag ->
                row(key = "tag:$tag") { TagRow(tag, onRemove = { editor.removeTag(tag) }) }
            }
            if (editor.tags.size >= MAX_TAGS) {
                row {
                    GroupRow {
                        HavenText(
                            stringResource(R.string.edit_tag_limit),
                            style = HavenTheme.type.value,
                            color = HavenTheme.colors.muted,
                        )
                    }
                }
            } else {
                // Keyed: a tag added before it must not take the field's focus away.
                row(key = "add") { AddTagField(text, enabled = query.isNotEmpty(), commit = commit) }
                suggestions.forEach { name ->
                    row(key = "suggestion:$name") {
                        GroupRow(
                            onClick = {
                                editor.addTag(name)
                                text.clearText()
                            },
                            icon = HavenIcon.Tag,
                            chevron = false,
                        ) { GroupRowText(name) }
                    }
                }
            }
        }
    }
}

@Composable
private fun TagRow(tag: String, onRemove: () -> Unit) {
    GroupRow(
        icon = HavenIcon.Tag,
        trailing = { HavenIconButton(HavenIcon.X, stringResource(R.string.edit_remove_tag, tag), onClick = onRemove) },
    ) { GroupRowText(tag) }
}

@Composable
private fun AddTagField(text: TextFieldState, enabled: Boolean, commit: () -> Unit) {
    val label = stringResource(R.string.edit_add_tag)
    var focused by remember { mutableStateOf(false) }
    Row(verticalAlignment = Alignment.CenterVertically) {
        HavenTextField(
            text,
            label,
            Modifier.weight(1f).onFocusChanged {
                // Leaving the field adds what was typed, as on the desktop.
                if (focused && !it.isFocused) commit()
                focused = it.isFocused
            },
            keyboardOptions = KeyboardOptions(
                capitalization = KeyboardCapitalization.None,
                autoCorrectEnabled = false,
                imeAction = ImeAction.Done,
            ),
            // Done adds the tag and keeps the keyboard up for the next one.
            onKeyboardAction = KeyboardActionHandler { commit() },
        )
        HavenIconButton(HavenIcon.Plus, label, onClick = commit, enabled = enabled)
    }
}
