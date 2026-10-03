@file:Suppress("MatchingDeclarationName")

package net.havenkeys.android.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color

/*
 * The desktop's named colour roles (apps/desktop/DESIGN.md; packages/ui
 * tokens; the per-theme layer at the top of apps/desktop/src/styles.css),
 * light and dark, following the system setting. Dynamic colour is off on
 * purpose: the vault must look like HavenKeys on every phone.
 *
 * Where the phone departs from the desktop (WCAG AA at the phone's
 * 13-16sp text; ContrastTest pins it):
 * - light muted #61706a (desktop #66756f: 4.45:1 on a group)
 * - light brassInk and digit #7d632f (desktop #8f7236: 4.17:1 on a group,
 *   4.0:1 on a brass-soft pill)
 * - dark onDanger #121a17 (desktop white: 3.0:1 on #e0775f)
 * New for the phone: onGlass (toast text; the glass is dark in both themes),
 * thumb (switch and slider thumbs, the desktop's paper white) and scrim (the
 * dim behind a sheet; the desktop has no sheets).
 */
@Immutable
data class HavenColors(
    val isDark: Boolean,
    /** The window ground: screens, unlock. */
    val pane: Color,
    /** A list's ground. */
    val list: Color,
    /** An inset group of rows. */
    val group: Color,
    /** The border of a group and the hairlines between its rows. */
    val groupLine: Color,
    /** An input's or a segmented control's track. */
    val field: Color,
    /** Something lifted: a sheet, a dialog, a menu, the segmented thumb. */
    val raised: Color,
    val hover: Color,
    val line: Color,
    /** The edge of anything interactive. */
    val lineStrong: Color,
    val text: Color,
    val textStrong: Color,
    val muted: Color,
    /** The fitting: controls and thin lines, never large surfaces. */
    val brass: Color,
    val brassHi: Color,
    /** Brass that carries text. */
    val brassInk: Color,
    /** The brass wash: pills, press feedback, a secure note's tile. */
    val brassSoft: Color,
    /** The selection wash: a focused field row. */
    val sel: Color,
    /** The one committing button: brass in dark, forest in light. */
    val primary: Color,
    val onPrimary: Color,
    val onBrass: Color,
    val ok: Color,
    val danger: Color,
    val onDanger: Color,
    /** The toast's ground, dark in both themes. */
    val glass: Color,
    val onGlass: Color,
    /** Digits inside a generated password. */
    val digit: Color,
    /** Symbols inside a generated password. */
    val symbol: Color,
    /** The monogram tile and its initial. */
    val avatarBg: Color,
    val avatarFg: Color,
    /** Switch and slider thumbs. */
    val thumb: Color,
    /** The dim behind a sheet. */
    val scrim: Color,
)

val DarkHavenColors = HavenColors(
    isDark = true,
    pane = Color(0xFF0F1614),
    list = Color(0xFF121B18),
    group = Color(0xFF152120),
    groupLine = Color(0x12E2EAE6),
    field = Color(0xFF0B110F),
    raised = Color(0xFF172320),
    hover = Color(0xFF1B2925),
    line = Color(0xFF1F2C28),
    lineStrong = Color(0xFF2C3D37),
    text = Color(0xFFE2EAE6),
    textStrong = Color(0xFFF3F7F5),
    muted = Color(0xFF86968F),
    brass = Color(0xFFC9A45C),
    brassHi = Color(0xFFE3C483),
    brassInk = Color(0xFFE3C483),
    brassSoft = Color(0x24C9A45C),
    sel = Color(0x26C9A45C),
    primary = Color(0xFFC9A45C),
    onPrimary = Color(0xFF121A17),
    onBrass = Color(0xFF121A17),
    ok = Color(0xFF5FA785),
    danger = Color(0xFFE0775F),
    onDanger = Color(0xFF121A17),
    glass = Color(0xE60F1614),
    onGlass = Color(0xFFF3F7F5),
    digit = Color(0xFFE3C483),
    symbol = Color(0xFF8FC4AD),
    avatarBg = Color(0xFF1D2B27),
    avatarFg = Color(0xFFE3C483),
    thumb = Color(0xFFFBFCFB),
    scrim = Color(0x8C000000),
)

val LightHavenColors = HavenColors(
    isDark = false,
    pane = Color(0xFFFFFFFF),
    list = Color(0xFFF4F7F5),
    group = Color(0xFFF3F6F4),
    groupLine = Color(0xFFE2E8E5),
    field = Color(0xFFFFFFFF),
    raised = Color(0xFFFFFFFF),
    hover = Color(0xFFE9EFEC),
    line = Color(0xFFE2E8E5),
    lineStrong = Color(0xFFCBD5D0),
    text = Color(0xFF1E2B27),
    textStrong = Color(0xFF121A17),
    muted = Color(0xFF61706A),
    brass = Color(0xFFC9A45C),
    brassHi = Color(0xFFE3C483),
    brassInk = Color(0xFF7D632F),
    brassSoft = Color(0x29C9A45C),
    sel = Color(0x2EC9A45C),
    primary = Color(0xFF16231F),
    onPrimary = Color(0xFFF1F5F3),
    onBrass = Color(0xFF121A17),
    ok = Color(0xFF3D6B58),
    danger = Color(0xFFB4412F),
    onDanger = Color(0xFFFFFFFF),
    glass = Color(0xF016231F),
    onGlass = Color(0xFFF3F7F5),
    digit = Color(0xFF7D632F),
    symbol = Color(0xFF3D6B58),
    avatarBg = Color(0xFF16231F),
    avatarFg = Color(0xFFE3C483),
    thumb = Color(0xFFFBFCFB),
    scrim = Color(0x4D16231F),
)
