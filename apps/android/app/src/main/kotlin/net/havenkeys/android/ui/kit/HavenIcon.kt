@file:Suppress("MaxLineLength") // Path data is copied verbatim from the desktop's Icon.tsx.

package net.havenkeys.android.ui.kit

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.isSpecified
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * The HavenKeys icon set: apps/desktop/src/components/Icon.tsx, path for
 * path (24 grid, 1.6 stroke, round caps and joins, drawn to sit with Hanken
 * Grotesk). It replaces material-icons-extended. A glyph the phone needs is
 * drawn to the same rules and added to the desktop set too; HavenIconTest
 * keeps the two identical.
 */
enum class HavenIcon(internal val path: String) {
    Lock("M7.5 10.5V8a4.5 4.5 0 0 1 9 0v2.5M6 10.5h12a1 1 0 0 1 1 1V19a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-7.5a1 1 0 0 1 1-1zM12 14.5v2"),
    Unlock("M7.5 10.5V8a4.5 4.5 0 0 1 8.7-1.6M6 10.5h12a1 1 0 0 1 1 1V19a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-7.5a1 1 0 0 1 1-1z"),
    Search("M10.5 17.5a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4.5-4.5"),
    Key("M14.5 3.5a6 6 0 1 1-5.2 9L4 17.8V20h2.5v-2h2v-2h2l1.4-1.4A6 6 0 0 1 14.5 3.5zM16 8h.01"),
    Note("M7 3.5h7l4.5 4.5v11.5a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1v-15a1 1 0 0 1 1-1zM13.5 3.5V8.5h5M9 12.5h6M9 16h4"),
    Grid("M5 4.5h5a.5.5 0 0 1 .5.5v5a.5.5 0 0 1-.5.5H5a.5.5 0 0 1-.5-.5V5a.5.5 0 0 1 .5-.5zM14 4.5h5a.5.5 0 0 1 .5.5v5a.5.5 0 0 1-.5.5h-5a.5.5 0 0 1-.5-.5V5a.5.5 0 0 1 .5-.5zM5 13.5h5a.5.5 0 0 1 .5.5v5a.5.5 0 0 1-.5.5H5a.5.5 0 0 1-.5-.5v-5a.5.5 0 0 1 .5-.5zM14 13.5h5a.5.5 0 0 1 .5.5v5a.5.5 0 0 1-.5.5h-5a.5.5 0 0 1-.5-.5v-5a.5.5 0 0 1 .5-.5z"),
    Qr("M4.5 4.5h6v6h-6zM13.5 4.5h6v6h-6zM4.5 13.5h6v6h-6zM7 7h1M16 7h1M7 16h1M13.5 13.5h2.5v2.5M19.5 13.5v2.5M13.5 19.5h2.5M18.5 18.5h1v1"),
    Dice("M12 3.5 19.5 7.5v9L12 20.5 4.5 16.5v-9L12 3.5zM4.5 7.5 12 11.5l7.5-4M12 11.5v9"),
    Gear("M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM10.3 3.5h3.4l.5 2.3 1.6.9 2.2-.8 1.7 3-1.8 1.5v1.8l1.8 1.5-1.7 3-2.2-.8-1.6.9-.5 2.3h-3.4l-.5-2.3-1.6-.9-2.2.8-1.7-3 1.8-1.5v-1.8L4.3 8.9l1.7-3 2.2.8 1.6-.9.5-2.3z"),
    Plus("M12 5v14M5 12h14"),
    Copy("M9 8.5h9a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1zM16 8.5V6a1 1 0 0 0-1-1H6a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h2"),
    Check("M5 12.5l4.5 4.5L19 7.5"),
    Eye("M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"),
    EyeOff("M4 4l16 16M10.2 5.7A9 9 0 0 1 12 5.5c6 0 9.5 6.5 9.5 6.5a16 16 0 0 1-2.9 3.7M6.6 6.9A15.6 15.6 0 0 0 2.5 12S6 18.5 12 18.5a8.8 8.8 0 0 0 4.6-1.3M9.9 9.9a3 3 0 0 0 4.2 4.2"),
    Edit("M4.5 19.5h4l10-10a2.1 2.1 0 0 0-3-3l-10 10v3zM14 8l3 3"),
    Trash("M4.5 7h15M9.5 7V5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v2M6.5 7l.9 12.1a1 1 0 0 0 1 .9h7.2a1 1 0 0 0 1-.9L17.5 7"),
    Globe("M12 20.5a8.5 8.5 0 1 0 0-17 8.5 8.5 0 0 0 0 17zM3.5 12h17M12 3.5c2.3 2.3 3.5 5.2 3.5 8.5s-1.2 6.2-3.5 8.5c-2.3-2.3-3.5-5.2-3.5-8.5S9.7 5.8 12 3.5z"),
    Refresh("M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5v4.5H15"),
    X("M6.5 6.5l11 11M17.5 6.5l-11 11"),
    Shield("M12 3.5l7 2.8V12c0 4.3-3 7.4-7 8.8-4-1.4-7-4.5-7-8.8V6.3l7-2.8z"),
    Clock("M12 20.5a8.5 8.5 0 1 0 0-17 8.5 8.5 0 0 0 0 17zM12 7.5V12l3 2"),
    ArrowRight("M5 12h14M13 6l6 6-6 6"),
    ChevronDown("M6.5 9.5 12 15l5.5-5.5"),
    ChevronUp("M6.5 14.5 12 9l5.5 5.5"),
    Grip("M8.4 6a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM14.4 6a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM8.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM14.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM8.4 18a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM14.4 18a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0z"),
    Cloud("M7 18.5h10a4 4 0 0 0 .6-8A5.5 5.5 0 0 0 7 9.5a4.5 4.5 0 0 0 0 9z"),
    CloudOff("M4 4l16 16M9 6.4A5.5 5.5 0 0 1 17.6 10.5 4 4 0 0 1 19.8 17M17 18.5H7a4.5 4.5 0 0 1-1.7-8.7"),
    Laptop("M5.5 6h13a1 1 0 0 1 1 1v8.5h-15V7a1 1 0 0 1 1-1zM2.5 18h19"),
    Alert("M12 4 21 19.5H3L12 4zM12 10v4M12 17h.01"),
    Download("M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19.5h14"),
    IdCard("M4.5 6h15a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1zM9 12a1.8 1.8 0 1 0 0-3.6A1.8 1.8 0 0 0 9 12zM6 15.5c.5-1.3 1.6-2 3-2s2.5.7 3 2M14 10h3.5M14 13.5h3.5"),
    Card("M4.5 6h15a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1zM3.5 10h17M7 14.5h3"),
    Printer("M7 9V4h10v5M7 17H5a1 1 0 0 1-1-1v-6a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-2M7 14h10v6H7z"),
    Home("M4.5 10.5 12 4.5l7.5 6V19a1 1 0 0 1-1 1H15v-5.5H9V20H5.5a1 1 0 0 1-1-1v-8.5z"),
    Items("M9 7h10.5M9 12h10.5M9 17h10.5M5 7h.01M5 12h.01M5 17h.01"),
    ChevronRight("M9.5 6.5 15 12l-5.5 5.5"),
    ChevronLeft("M14.5 6.5 9 12l5.5 5.5"),
    More("M5.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM11.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM17.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0z"),
    ;

    /** This glyph's key in Icon.tsx. */
    internal val desktopName: String get() = name.replaceFirstChar { it.lowercaseChar() }

    internal val vector: ImageVector by lazy { glyph(name, path) }
}

private const val GRID = 24f
private const val STROKE = 1.6f

private fun glyph(name: String, path: String): ImageVector =
    ImageVector.Builder(
        name = name,
        defaultWidth = GRID.dp,
        defaultHeight = GRID.dp,
        viewportWidth = GRID,
        viewportHeight = GRID,
    ).addPath(
        pathData = PathParser().parsePathString(path).toNodes(),
        fill = null,
        stroke = SolidColor(Color.Black),
        strokeLineWidth = STROKE,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
    ).build()

/**
 * One glyph. With a [contentDescription] it is an image TalkBack reads;
 * with null it is decoration (the control around it carries the label).
 */
@Composable
fun IconGlyph(
    icon: HavenIcon,
    contentDescription: String?,
    modifier: Modifier = Modifier,
    tint: Color = Color.Unspecified,
    size: Dp = 20.dp,
) {
    Image(
        painter = rememberVectorPainter(icon.vector),
        contentDescription = contentDescription,
        modifier = modifier.size(size),
        colorFilter = ColorFilter.tint(if (tint.isSpecified) tint else contentColor()),
    )
}

@PreviewLightDark
@Composable
private fun HavenIconPreview() {
    KitPreview {
        HavenIcon.entries.chunked(8).forEach { row ->
            Row {
                row.forEach { IconGlyph(it, contentDescription = null, size = 28.dp) }
            }
        }
    }
}
