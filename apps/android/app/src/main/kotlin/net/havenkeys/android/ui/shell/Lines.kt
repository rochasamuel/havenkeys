package net.havenkeys.android.ui.shell

import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** A screen's large title (Items, a category, Settings): serif, a heading for TalkBack. */
@Composable
fun LargeTitle(text: String, modifier: Modifier = Modifier) {
    HavenText(
        text,
        modifier.padding(top = 8.dp, bottom = 12.dp).semantics { heading() },
        style = HavenTheme.type.headline,
        color = HavenTheme.colors.textStrong,
    )
}

/** A failed load or sync in the app's words for its code; Rust's own detail is never shown. */
@Composable
fun ErrorLine(code: String, modifier: Modifier = Modifier) {
    HavenText(
        stringResource(errorText(code)),
        modifier.padding(vertical = 8.dp).semantics { liveRegion = LiveRegionMode.Polite },
        color = HavenTheme.colors.danger,
    )
}

/** What an empty list says, lined up with the rows' text. */
@Composable
fun EmptyLine(text: String, modifier: Modifier = Modifier) {
    HavenText(
        text,
        modifier.padding(horizontal = HavenSpacing.rowX, vertical = 8.dp),
        color = HavenTheme.colors.muted,
    )
}
