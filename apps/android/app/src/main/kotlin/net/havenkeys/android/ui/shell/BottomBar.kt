package net.havenkeys.android.ui.shell

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.kit.HavenPress
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

private val BarHeight = 60.dp

/**
 * Home, Items, Settings (spec §6.4): each a tab for TalkBack, the selected
 * one in strong ink with a brass marker under it, a light tick on a change.
 * A tap on the current tab is reported too (the shell pops it to its root).
 */
@Composable
fun BottomBar(selected: Tab, onSelect: (Tab) -> Unit, modifier: Modifier = Modifier) {
    val colors = HavenTheme.colors
    val haptics = LocalHapticFeedback.current
    Column(modifier.fillMaxWidth().background(colors.pane)) {
        Box(Modifier.fillMaxWidth().height(1.dp).background(colors.line))
        Row(Modifier.fillMaxWidth().selectableGroup()) {
            Tab.entries.forEach { tab ->
                val isSelected = tab == selected
                val ink = if (isSelected) colors.textStrong else colors.muted
                val marker by animateColorAsState(
                    if (isSelected) colors.brass else Color.Transparent,
                    HavenTheme.motion.fadeSpec(),
                    label = "tab-marker",
                )
                Column(
                    Modifier
                        .weight(1f)
                        .heightIn(min = BarHeight)
                        .selectable(
                            selected = isSelected,
                            interactionSource = null,
                            indication = HavenPress,
                            role = Role.Tab,
                        ) {
                            if (!isSelected) haptics.performHapticFeedback(HapticFeedbackType.SegmentTick)
                            onSelect(tab)
                        }
                        .padding(top = 8.dp, bottom = 6.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(3.dp),
                ) {
                    IconGlyph(tab.icon, contentDescription = null, tint = ink, size = 24.dp)
                    HavenText(stringResource(tab.label), style = HavenTheme.type.label, color = ink)
                    Box(Modifier.size(width = 18.dp, height = 2.dp).clip(HavenShape.pill).background(marker))
                }
            }
        }
    }
}
