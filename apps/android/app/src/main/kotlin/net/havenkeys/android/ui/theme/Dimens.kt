package net.havenkeys.android.ui.theme

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.dp

/** The desktop's radii (spec §5.1: 8, 10, 12, 14, 18 dp). */
object HavenRadius {
    /** Small controls, menu rows, the segmented thumb. */
    val control = 8.dp

    /** Buttons, monogram tiles, the segmented track. */
    val row = 10.dp

    /** Inset groups and field rows. */
    val group = 12.dp

    /** Menus, the generator's output. */
    val output = 14.dp

    /** Sheets and dialogs. */
    val sheet = 18.dp
}

object HavenShape {
    val control: Shape = RoundedCornerShape(HavenRadius.control)
    val row: Shape = RoundedCornerShape(HavenRadius.row)
    val group: Shape = RoundedCornerShape(HavenRadius.group)
    val output: Shape = RoundedCornerShape(HavenRadius.output)
    val dialog: Shape = RoundedCornerShape(HavenRadius.sheet)

    /** A bottom sheet: rounded where it meets the screen above it. */
    val sheet: Shape = RoundedCornerShape(topStart = HavenRadius.sheet, topEnd = HavenRadius.sheet)

    /** Pills, the toast, round buttons. */
    val pill: Shape = RoundedCornerShape(percent = 50)
}

/** The phone's rhythm: the desktop's density, opened up for a thumb. */
object HavenSpacing {
    /** A screen's side margin. */
    val gutter = 16.dp

    /** Horizontal padding inside a row; also where a row's hairline starts. */
    val rowX = 16.dp

    /** Between groups. */
    val groupGap = 24.dp

    /** The smallest touch target (spec §5.2). */
    val touch = 48.dp

    /** A group row's minimum height. */
    val rowMin = 52.dp

    /** An item row's minimum height (40dp tile plus padding). */
    val itemRowMin = 64.dp

    /** A list row's monogram tile. */
    val tile = 40.dp
}
