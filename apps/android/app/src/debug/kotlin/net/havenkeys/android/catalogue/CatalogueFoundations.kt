package net.havenkeys.android.catalogue

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

@Composable
internal fun Foundations() {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        SectionHeader("Colours")
        Swatches()
        SectionHeader("Type")
        TypeSamples()
        SectionHeader("Icons")
        HavenIcon.entries.chunked(6).forEach { row ->
            Row {
                row.forEach { icon ->
                    Column(Modifier.width(56.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                        IconGlyph(icon, contentDescription = null, size = 24.dp)
                        HavenText(icon.name, style = HavenTheme.type.pill, color = HavenTheme.colors.muted)
                    }
                }
            }
        }
    }
}

@Composable
private fun Swatches() {
    val c = HavenTheme.colors
    val swatches = listOf(
        "pane" to c.pane, "list" to c.list, "group" to c.group, "field" to c.field,
        "raised" to c.raised, "line" to c.line, "lineStrong" to c.lineStrong, "text" to c.text,
        "textStrong" to c.textStrong, "muted" to c.muted, "brass" to c.brass, "brassHi" to c.brassHi,
        "brassInk" to c.brassInk, "brassSoft" to c.brassSoft, "primary" to c.primary, "ok" to c.ok,
        "danger" to c.danger, "glass" to c.glass, "avatar" to c.avatarBg, "digit" to c.digit,
        "symbol" to c.symbol, "sel" to c.sel, "thumb" to c.thumb, "scrim" to c.scrim,
    )
    swatches.chunked(4).forEach { row ->
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            row.forEach { (name, color) -> Swatch(name, color) }
        }
    }
}

@Composable
private fun Swatch(name: String, color: Color) {
    Column(Modifier.width(72.dp)) {
        Box(
            Modifier.size(40.dp).clip(HavenShape.control).background(color)
                .border(1.dp, HavenTheme.colors.lineStrong, HavenShape.control),
        )
        HavenText(name, style = HavenTheme.type.pill, color = HavenTheme.colors.muted)
    }
}

@Composable
private fun TypeSamples() {
    val t = HavenTheme.type
    val c = HavenTheme.colors
    listOf(
        "Display: HavenKeys is locked." to t.display,
        "Headline: GitHub" to t.headline,
        "Title: New item" to t.title,
        "Small title: Nothing here yet" to t.titleSmall,
        "Body: the reading size, for sentences that explain." to t.body,
        "Value: sam@example.com" to t.value,
        "Row title: GitHub" to t.rowTitle,
        "Row subtitle: sam@example.com" to t.rowSubtitle,
        "Label: Username" to t.label,
        "Group title: Frequently used" to t.groupTitle,
        "Button: Unlock" to t.button,
        "Pill: This device" to t.pill,
    ).forEach { (text, style) -> HavenText(text, style = style, color = c.textStrong) }
    HavenText("k7#Rq2!vL9pX", style = t.secret, color = c.textStrong)
    HavenText("381 492", style = t.code, color = c.brassInk)
    HavenText("••••••••••••", style = t.masked, color = c.textStrong)
}
