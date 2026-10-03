package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenTheme

/** A preview frame: the theme (light or dark from @PreviewLightDark) on the window ground. */
@Composable
internal fun KitPreview(content: @Composable ColumnScope.() -> Unit) {
    HavenTheme {
        Column(
            Modifier.background(HavenTheme.colors.pane).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            content = content,
        )
    }
}
