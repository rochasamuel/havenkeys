package net.havenkeys.android.ui.edit

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.ChoiceSheet
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowField
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SectionHeader
import uniffi.havenkeys_mobile.MatchKind

/** The three rules in the order the desktop lists them; their labels are sentences, hence a sheet. */
private val matchOrder = listOf(MatchKind.DOMAIN, MatchKind.ORIGIN, MatchKind.EXACT)

private fun matchLabel(match: MatchKind): Int = when (match) {
    MatchKind.DOMAIN -> R.string.edit_match_domain
    MatchKind.ORIGIN -> R.string.edit_match_origin
    MatchKind.EXACT -> R.string.edit_match_exact
}

/** A login's websites: each address with Remove, its match rule under it, then Add website. */
@Composable
internal fun Websites(editor: EditorState) {
    var choosing by remember { mutableStateOf<WebsiteRow?>(null) }
    Column {
        SectionHeader(stringResource(R.string.edit_websites))
        InsetGroup {
            editor.websites.forEach { site ->
                row { key(site) { WebsiteEditor(site, onRemove = { editor.websites.remove(site) }) } }
                row { key(site) { MatchRow(site.match, onClick = { choosing = site }) } }
            }
            row {
                GroupRow(onClick = editor::addWebsite, icon = HavenIcon.Plus, chevron = false) {
                    GroupRowText(stringResource(R.string.edit_add_website))
                }
            }
        }
    }
    choosing?.let { site ->
        ChoiceSheet(
            title = stringResource(R.string.edit_match),
            values = matchOrder,
            selected = site.match,
            label = { stringResource(matchLabel(it)) },
            onSelect = { site.match = it },
            onDismiss = { choosing = null },
        )
    }
}

@Composable
private fun WebsiteEditor(site: WebsiteRow, onRemove: () -> Unit) {
    val url = rememberDraftText(site, initial = { site.url }, onEdit = { site.url = it })
    Row(verticalAlignment = Alignment.CenterVertically) {
        HavenTextField(
            url,
            stringResource(R.string.field_website),
            Modifier.weight(1f),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
        )
        HavenIconButton(HavenIcon.X, stringResource(R.string.edit_remove_website), onClick = onRemove)
    }
}

@Composable
private fun MatchRow(match: MatchKind, onClick: () -> Unit) {
    GroupRow(onClick = onClick) {
        GroupRowField(stringResource(R.string.edit_match), stringResource(matchLabel(match)))
    }
}
