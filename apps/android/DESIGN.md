---
name: HavenKeys (Android app)
description: The desktop's Vault Room in the hand; forest grounds, one brass fitting, serif titles and inset groups of hairline rows, rebuilt on Compose foundation for a thumb, with no Material underneath.
colors:
  pane: "#0f1614"
  pane-light: "#ffffff"
  list: "#121b18"
  list-light: "#f4f7f5"
  group: "#152120"
  group-light: "#f3f6f4"
  group-line: "rgba(226, 234, 230, 0.07)"
  group-line-light: "#e2e8e5"
  field: "#0b110f"
  field-light: "#ffffff"
  raised: "#172320"
  raised-light: "#ffffff"
  hover: "#1b2925"
  hover-light: "#e9efec"
  line: "#1f2c28"
  line-light: "#e2e8e5"
  line-strong: "#2c3d37"
  line-strong-light: "#cbd5d0"
  text: "#e2eae6"
  text-light: "#1e2b27"
  text-strong: "#f3f7f5"
  text-strong-light: "#121a17"
  muted: "#86968f"
  muted-light: "#606f69"
  brass: "#c9a45c"
  brass-hi: "#e3c483"
  brass-ink: "#e3c483"
  brass-ink-light: "#7d632f"
  brass-soft: "rgba(201, 164, 92, 0.14)"
  brass-soft-light: "rgba(201, 164, 92, 0.16)"
  sel: "rgba(201, 164, 92, 0.15)"
  sel-light: "rgba(201, 164, 92, 0.18)"
  primary: "#c9a45c"
  primary-light: "#16231f"
  on-primary: "#121a17"
  on-primary-light: "#f1f5f3"
  on-brass: "#121a17"
  ok: "#5fa785"
  ok-light: "#3d6b58"
  danger: "#e0775f"
  danger-light: "#b4412f"
  on-danger: "#121a17"
  on-danger-light: "#ffffff"
  glass: "rgba(15, 22, 20, 0.95)"
  glass-light: "rgba(22, 35, 31, 0.95)"
  glass-rim: "rgba(226, 234, 230, 0.1)"
  on-glass: "#f3f7f5"
  digit: "#e3c483"
  digit-light: "#7d632f"
  symbol: "#8fc4ad"
  symbol-light: "#3d6b58"
  avatar-bg: "#1d2b27"
  avatar-bg-light: "#16231f"
  avatar-fg: "#e3c483"
  thumb: "#fbfcfb"
  scrim: "rgba(0, 0, 0, 0.55)"
  scrim-light: "rgba(22, 35, 31, 0.3)"
typography:
  display:
    fontFamily: "HavenKeys Serif (Source Serif 4 subset)"
    fontSize: "32sp"
    fontWeight: 460
    lineHeight: "36sp"
    letterSpacing: "-0.015em"
  headline:
    fontFamily: "HavenKeys Serif (Source Serif 4 subset)"
    fontSize: "28sp"
    fontWeight: 480
    lineHeight: "32sp"
    letterSpacing: "-0.015em"
  title:
    fontFamily: "HavenKeys Serif (Source Serif 4 subset)"
    fontSize: "22sp"
    fontWeight: 480
    lineHeight: "28sp"
    letterSpacing: "-0.012em"
  title-small:
    fontFamily: "HavenKeys Serif (Source Serif 4 subset)"
    fontSize: "18sp"
    fontWeight: 500
    lineHeight: "24sp"
  monogram:
    fontFamily: "HavenKeys Serif (Source Serif 4 subset)"
    fontSize: "18sp"
    fontWeight: 520
    lineHeight: "22sp"
  body:
    fontFamily: "Hanken Grotesk"
    fontSize: "15sp"
    fontWeight: 400
    lineHeight: "22sp"
    fontFeature: "'ss01'"
  value:
    fontFamily: "Hanken Grotesk"
    fontSize: "16sp"
    fontWeight: 400
    lineHeight: "22sp"
    fontFeature: "'ss01'"
  row-title:
    fontFamily: "Hanken Grotesk"
    fontSize: "16sp"
    fontWeight: 600
    lineHeight: "21sp"
    fontFeature: "'ss01'"
  row-subtitle:
    fontFamily: "Hanken Grotesk"
    fontSize: "14sp"
    fontWeight: 400
    lineHeight: "19sp"
    fontFeature: "'ss01'"
  label:
    fontFamily: "Hanken Grotesk"
    fontSize: "13sp"
    fontWeight: 560
    lineHeight: "17sp"
    fontFeature: "'ss01'"
  group-title:
    fontFamily: "Hanken Grotesk"
    fontSize: "14sp"
    fontWeight: 600
    lineHeight: "19sp"
    fontFeature: "'ss01'"
  button:
    fontFamily: "Hanken Grotesk"
    fontSize: "16sp"
    fontWeight: 550
    lineHeight: "20sp"
    fontFeature: "'ss01'"
  pill:
    fontFamily: "Hanken Grotesk"
    fontSize: "12sp"
    fontWeight: 600
    lineHeight: "16sp"
    fontFeature: "'ss01'"
  masked:
    fontFamily: "Hanken Grotesk"
    fontSize: "16sp"
    fontWeight: 500
    lineHeight: "22sp"
    letterSpacing: "0.18em"
  secret:
    fontFamily: "JetBrains Mono"
    fontSize: "16sp"
    fontWeight: 500
    lineHeight: "22sp"
    letterSpacing: "0.02em"
  code:
    fontFamily: "JetBrains Mono"
    fontSize: "22sp"
    fontWeight: 500
    lineHeight: "28sp"
    letterSpacing: "0.05em"
    fontFeature: "'tnum'"
rounded:
  control: "8dp"
  row: "10dp"
  group: "12dp"
  output: "14dp"
  sheet: "18dp"
  pill: "50%"
spacing:
  gutter: "16dp"
  row-x: "16dp"
  group-gap: "24dp"
  touch: "48dp"
  row-min: "52dp"
  item-row-min: "64dp"
  tile: "40dp"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    typography: "{typography.button}"
    rounded: "{rounded.row}"
    padding: "12dp 18dp"
    height: "48dp"
  button-primary-light:
    backgroundColor: "{colors.primary-light}"
    textColor: "{colors.on-primary-light}"
    rounded: "{rounded.row}"
    padding: "12dp 18dp"
    height: "48dp"
  button-secondary:
    backgroundColor: "transparent"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.row}"
    padding: "12dp 18dp"
    height: "48dp"
  button-quiet:
    backgroundColor: "transparent"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.row}"
    padding: "12dp 18dp"
    height: "48dp"
  button-danger:
    backgroundColor: "{colors.danger}"
    textColor: "{colors.on-danger}"
    rounded: "{rounded.row}"
    padding: "12dp 18dp"
    height: "48dp"
  button-danger-light:
    backgroundColor: "{colors.danger-light}"
    textColor: "{colors.on-danger-light}"
  icon-button:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.pill}"
    size: "48dp"
  copy-button:
    textColor: "{colors.muted}"
    rounded: "{rounded.pill}"
    size: "48dp"
  add-button:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.pill}"
    size: "56dp"
  pill:
    backgroundColor: "{colors.brass-soft}"
    textColor: "{colors.brass-ink}"
    typography: "{typography.pill}"
    rounded: "{rounded.pill}"
    padding: "3dp 9dp"
  pill-outline:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.pill}"
    padding: "3dp 9dp"
  inset-group:
    backgroundColor: "{colors.group}"
    rounded: "{rounded.group}"
  group-row:
    textColor: "{colors.text-strong}"
    typography: "{typography.value}"
    padding: "10dp 8dp 10dp 16dp"
    height: "52dp"
  item-row:
    textColor: "{colors.text-strong}"
    typography: "{typography.row-title}"
    padding: "10dp 16dp"
    height: "64dp"
  item-tile:
    backgroundColor: "{colors.avatar-bg}"
    textColor: "{colors.avatar-fg}"
    typography: "{typography.monogram}"
    rounded: "10dp"
    size: "40dp"
  item-tile-soft:
    backgroundColor: "{colors.brass-soft}"
    textColor: "{colors.brass-ink}"
  section-header:
    textColor: "{colors.muted}"
    typography: "{typography.group-title}"
    padding: "8dp 0 8dp 16dp"
    height: "36dp"
  field-row:
    backgroundColor: "transparent"
    textColor: "{colors.text-strong}"
    typography: "{typography.value}"
    padding: "8dp 16dp"
    height: "52dp"
  field-row-focused:
    backgroundColor: "{colors.sel}"
  secret-field:
    textColor: "{colors.text-strong}"
    typography: "{typography.secret}"
    height: "52dp"
  switch:
    backgroundColor: "{colors.line-strong}"
    rounded: "{rounded.pill}"
    width: "44dp"
    height: "26dp"
  switch-on:
    backgroundColor: "{colors.brass}"
  slider:
    backgroundColor: "{colors.line-strong}"
    height: "48dp"
  segmented:
    backgroundColor: "{colors.field}"
    rounded: "{rounded.row}"
    padding: "4dp 3dp"
    height: "48dp"
  segmented-light:
    backgroundColor: "{colors.hover-light}"
  segmented-selected:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text-strong}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
  sheet:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.sheet}"
    padding: "0 16dp 16dp"
  dialog:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.sheet}"
    padding: "24dp"
    width: "360dp"
  menu:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.output}"
    padding: "6dp 0"
    width: "200dp"
  menu-row:
    typography: "{typography.body}"
    padding: "0 14dp"
    height: "48dp"
  toast:
    backgroundColor: "{colors.glass}"
    textColor: "{colors.on-glass}"
    typography: "{typography.body}"
    rounded: "{rounded.pill}"
    padding: "10dp 16dp 10dp 13dp"
  progress-ring:
    textColor: "{colors.brass}"
    size: "28dp"
---

# Design System: HavenKeys (Android app)

## Overview

**Creative North Star: "The Vault Room, in the hand"**

The phone is the desktop's Vault Room carried in one hand: the same forest-green grounds, the same single brass fitting, serif titles over a warm grotesque, and secrets alone in mono. Where the desktop arranges the room in three panes, the phone stacks it: a screen is a column of inset groups of hairline-separated rows on the window ground, framed by `HavenScaffold` between the status bar and the navigation bar, with a floating add button and a glass toast above the bottom edge. Overlays are the room's furniture: a bottom sheet that springs up on a dim, a dialog, a small menu under its trigger.

The kit (`ui/kit`) is built on `androidx.compose.foundation` only. Nothing in it is Material: there is no ripple (press is a 0.97 scale and a brass-soft wash), no Material type scale, no Material sheet; the switch, slider and segmented control are the desktop's shapes at phone size, and the icon set is the desktop's `Icon.tsx` path for path. Dynamic colour is off on purpose: the vault looks like HavenKeys on every phone. Light and dark follow the system setting.

The desktop's density is opened up for a thumb: reading text rises from 13.5px to 15sp, rows to 52dp, every control is at least 48dp. Motion is physical and named in Apple's terms (four springs and one fade), and the system's "Remove animations" cuts all of it.

**Key Characteristics:**
- Inset groups (12dp) of hairline rows; hairlines start where the text of the row below starts.
- Serif titles (HavenKeys Serif) and monogram tiles; Hanken Grotesk for everything else; JetBrains Mono for secrets and codes.
- Brass as a fitting: the dark primary, focus, the switch and slider fill, the ring, brass-ink text; the light primary is forest.
- No ripple, no Material: press scales to 0.97 with a brass-soft wash.
- Every control at least 48dp; text sizes in sp, so they follow the system font size.
- Sheets, dialogs and menus live in secure windows (FLAG_SECURE, no autofill, obscured taps dropped).

## Colors

The desktop's named roles in light and dark (`ui/theme/Color.kt`, `HavenColors`), with three departures for contrast at the phone's 13 to 16sp text and four roles the desktop does not need.

### Primary
- **Fitting Brass** (brass): the dark primary button and add button, the focus ring of a field row, the text cursor and selection handles, the switch track when on, the slider fill, the progress ring. Always a control or a thin line.
- **Lit Brass / Brass Ink** (brass-hi, brass-ink, brass-ink-light): brass that carries text: a pill's label, a section action ("Clear"), the glyph in a soft tile. In light theme it darkens to `brass-ink-light`.
- **Brass Wash / Selection Wash** (brass-soft, sel): press feedback, pills, the secure-note tile; a focused field row.
- **Forest Primary** (primary-light, on-primary-light): the one committing button in light theme.

### Secondary
- **Cipher Green** (symbol, symbol-light) and **Digit Brass** (digit, digit-light): symbols and digits inside a generated password, so they read apart from letters.

### Tertiary
- **Leaf OK** (ok, ok-light): the copy check, the toast's done glyph.
- **Ember** (danger, danger-light, with on-danger): errors, the warning ring, destructive buttons and menu rows.

### Neutral
- **Window Green** (pane): the screen ground. White in light.
- **List Green** (list), **Group Green** (group) with **Group Hairline** (group-line): list grounds, inset groups and the hairlines between their rows.
- **Field Green** (field): the segmented track in dark (light uses `hover-light`, because the light field is white like the pane).
- **Raised** (raised): sheets, dialogs, menus, the segmented thumb.
- **Hairline / Strong Hairline** (line, line-strong): the progress ring's track; the edge of anything interactive (secondary button, menu border, sheet handle, switch track off, slider track).
- **Bone / Bright Bone / Lichen** (text, text-strong, muted): body, titles and values, labels and meta.
- **Avatar** (avatar-bg, avatar-fg): the monogram tile; forest with a brass initial in both themes.
- **Glass** (glass, glass-light, on-glass, glass-rim): the toast, dark in both themes, with a hairline rim.
- **Thumb** (thumb): the paper-white switch and slider thumbs.
- **Scrim** (scrim, scrim-light): the dim behind a sheet and a dialog (black at 55% in dark, forest at 30% in light).

### Contrast departures from the desktop
`ContrastTest` pins these. Each desktop value fails WCAG AA at the phone's text sizes:
- **Light muted** `#606f69` (desktop `#66756f`: 4.45:1 on a group; phone 4.86:1 on a group and 4.53:1 on the hover ground of the search pill and segmented track, where stage 2's `#61706a` gave 4.47:1).
- **Light brass ink and digit** `#7d632f` (desktop `#8f7236`: 4.17:1 on a group and 4.0:1 on a brass-soft pill; phone 5.22:1 on a group).
- **Dark on-danger** `#121a17` (desktop white on `#e0775f`: 3.01:1; phone 5.88:1). Dark danger buttons carry dark text.

New for the phone: `on-glass` (toast text), `thumb` (the desktop's paper white as a role) and `scrim` (the desktop has no sheets).

### Named Rules
**The One Fitting Rule.** Brass is a fitting, not a finish: controls and thin lines, never large surfaces. One primary per screen: brass in dark, forest in light. The dark add button is a brass disc because it is the screen's one primary; a screen that shows it shows no primary `HavenButton`.

## Typography

**Display Font:** HavenKeys Serif (Source Serif 4, subset and renamed under the OFL), roman 400 to 520 at optical size 24, one italic at 400
**Body Font:** Hanken Grotesk (variable, 400 to 660), stylistic set ss01
**Label/Mono Font:** JetBrains Mono 500

**Character:** The serif names things, Hanken does the work, mono is evidence that a string is exact. Sizes are sp, so all text follows the system font size; only the monogram initial is sized from its tile so it never spills out.

### Hierarchy
- **Display** (serif 460, 32sp / 36, -0.015em): the unlock headline; its one brass italic is a span there.
- **Headline** (serif 480, 28sp / 32, -0.015em): item and editor titles, a list screen's large title, the catalogue's section heads.
- **Title** (serif 480, 22sp / 28, -0.012em): sheet and dialog titles.
- **Small title** (serif 500, 18sp / 24): empty-state lines, small notices.
- **Monogram** (serif 520, 45% of the tile): the initial in an item tile.
- **Body** (400, 15sp / 22): the reading size; dialog messages, menu rows, toast text.
- **Value** (400, 16sp / 22): a row's value, a field's typed text, a trailing count.
- **Row title** (600, 16sp / 21) over **row subtitle** (400, 14sp / 19, muted): item rows.
- **Label** (560, 13sp / 17, muted): the label above a value; segmented options.
- **Group title** (600, 14sp / 19, muted, sentence case): section headers and their action.
- **Button** (550, 16sp / 20); **Pill** (600, 12sp / 16).
- **Masked** (Hanken 500, 16sp, 0.18em): the fixed row of dots for a hidden value.
- **Secret** (mono 500, 16sp / 22, 0.02em): a revealed password, key or setup string; the secret field.
- **Code** (mono 500, 22sp / 28, 0.05em, tabular): a one-time code, so it does not jitter.

### The font files
`res/font/hanken_grotesk.ttf`, `havenkeys_serif.ttf`, `havenkeys_serif_italic.ttf` and `jetbrains_mono.ttf` are committed; nothing is downloaded at run or build time. `ui/theme/Fonts.kt` declares each weight the desktop uses once with its axis value (Hanken 400, 500, 550, 560, 600, 640, 650, 660; serif 400, 460, 470, 480, 500, 520), so the variable file draws exactly that weight. The subset is Latin (Basic Latin through Latin Extended-B, punctuation, euro, arrows, bullet and the mask dot); other glyphs fall back to the system fonts.

To rebuild, from the repository root: `uvx --from fonttools==4.66.1 python scripts/build-android-fonts.py`. It downloads the three OFL families from google/fonts at a pinned commit, checks their SHA-256, subsets, pins the unused axes (serif at opsz 24; mono at 500), renames the serif files "HavenKeys Serif" (Source is a Reserved Font Name, and a subset is a Modified Version) while keeping copyright and trademark notices, and keeps timestamps so a rerun gives the same bytes. Licences are in `THIRD-PARTY-NOTICES.md`.

### Named Rules
**The Literal String Rule.** JetBrains Mono is for secrets, one-time codes, keys and setup strings. Never for labels, headings or counts.

**The Sentence Case Rule.** Labels, group titles and section actions are sentence case at their size; no all-caps tracked labels.

## Layout

A screen is one column on the pane: a 16dp gutter on each side, inset groups 24dp apart, each with a section header (36dp minimum, text 16dp in so it lines up with the rows' text). Rows pad 16dp at the start; a group row is at least 52dp, an item row at least 64dp (a 40dp tile, 12dp gap, then the text). A row that ends in a control keeps only 8dp at its end, the room a 48dp control fills; a trailing value adds the difference so it sits on the same 16dp margin as the text.

Every control is at least 48dp in both directions: icon and copy buttons are 48dp circles, the switch is 52 x 48, each segment and the slider are 48dp tall with their track drawn inset, menu rows and section actions are 48dp. Pills are markers, not controls, and have no target.

`HavenScaffold` handles insets: the top bar sits under the status bar, the bottom bar over the navigation bar (or a spacer of its height), and the keyboard pushes the whole frame up (`imePadding`). With a floating button the content gets 88dp of bottom room (56dp button, 16dp margin, 16dp air); the button sits 16dp from the bottom end; the toast sits 22dp above that room. Sheets pad for the navigation bar themselves. Dialogs are at most 360dp wide; menus 200 to 280dp.

## Elevation & Depth

Depth is tonal at rest: pane, group and raised step through greens, and groups sit flat on a hairline border. Only what floats casts a shadow: the sheet (24dp, top corners), the dialog (24dp), the menu (16dp, with a line-strong border), the toast (16dp, with its glass rim) and the add button (10dp). The switch and slider thumbs and the segmented thumb carry the desktop's small soft shadows drawn with a Paint shadow layer (switch 0 1dp 3dp at 35%, slider 0 1dp 4dp at 45%, segment 0 1dp 3dp at 25% under its hairline), so they ring the whole thumb and lift a white thumb off a white ground. Focus on a field is the selection wash plus a 1.5dp brass border, not a shadow.

The sheet draws its own scrim, faded with how far it has risen; the dialog's window dims by the scrim's alpha, so both darken the screen alike instead of the platform's 60% black.

### Named Rules
**The Floating-Only Shadow Rule.** Only sheet, dialog, menu, toast and add button cast elevation shadows. Groups, rows and the screen stay flat with a hairline.

## Shapes

Five radii, sized to the object (`HavenRadius`): 8dp small controls, menu rows and the segmented thumb; 10dp buttons and the segmented track; 12dp inset groups and field rows; 14dp menus and the generator's output; 18dp sheets (top corners only) and dialogs. Pills, the toast, icon buttons, the copy button and the add button are fully round. The item tile's corner is a quarter of its size (10dp at 40dp). The sheet handle is a 36 x 5dp pill. Borders and hairlines are 1dp; the focused field border is 1.5dp. Icons are one family on a 24 grid with a 1.6 stroke and round caps and joins, drawn at 16 to 26dp.

## Motion

Named springs in Apple's terms (SwiftUI response and damping fraction), converted for Compose as stiffness = (2π / response)² and dampingRatio = damping (`ui/theme/Motion.kt`, `HavenSprings`):
- **smooth** (response 0.3 s, damping 1.0; Compose stiffness ≈ 438.6, damping ratio 1.0): pushes, tab changes, pull to refresh settling; no bounce.
- **sheet** (0.3 s, 0.86; stiffness ≈ 438.6, ratio 0.86): the sheet rising, closing and settling after a drag; the menu scaling in from its top end (from 0.96).
- **toast** (0.28 s, 0.8; stiffness ≈ 503.6, ratio 0.8): the toast rising half its height.
- **press** (0.15 s, 1.0; stiffness ≈ 1755, ratio 1.0): press scale and wash, the switch thumb.

The **fade** is 120ms on the house curve `cubic-bezier(0.32, 0.72, 0, 1)`: fades in and out, colour changes, the copy glyph's cross-fade to a check, the switch track colour, the segmented thumb. Durations from the shared tokens run on the mechanical curve `cubic-bezier(0.3, 0.7, 0.2, 1)`: snap 160ms, tick 250ms (the one-time code ring's step), seal 260ms (unlock to vault). A spinner turns once every 900ms, linear. The springs are quicker than SwiftUI's defaults on purpose: the slower first set (smooth at 0.5 s) read as the app holding the user back. The **stagger** between items that arrive in sequence is 20ms (sheet tiles, Home's groups); it is a theme constant that the screens of stage 3 will use.

`HavenMotion` follows the system animator scale live. Under "Remove animations" (scale 0) every spring and tween becomes an instant cut, and `ProgressRing` stops spinning and draws a still arc.

### Named Rules
**The Stillness Rule.** "Remove animations" cuts everything: springs, fades, the press, the toast, the sheet, and the spinner. Nothing waits on an animation to finish.

## Components

### Structure
- **HavenScaffold:** a screen's frame on the pane: top bar under the status bar, content, optional bottom bar over the navigation bar, the floating button at the bottom end and the toast host above the bottom edge; the keyboard pushes it up. It hands the content the bottom room the floating button needs. TalkBack reads only what is placed in it.
- **SectionHeader:** a group's title, muted 14sp sentence case, 16dp in; optional trailing action in brass ink as a 48dp text button ("Clear"). TalkBack reads the title as a heading and the action as a button.
- **InsetGroup:** 12dp rounded group ground with a hairline border; rows never get a card of their own. Each row reports where its text starts and the hairline above it starts there (16dp, 52dp past a glyph, 68dp past an item tile), mirrored in right-to-left. Hairlines carry no text, so TalkBack should skip them.
- **GroupRow:** one row of a group: optional 22dp muted glyph, the text (`GroupRowText` title and muted detail, or `GroupRowField` label above value), then trailing controls (`TrailingText`, `CopyButton`, …) or a muted chevron when it opens something and has no trailing content. A tappable row is one button for TalkBack, with an optional click label. `GroupRow` without an action merges its text (a label and its value) into one TalkBack stop; its trailing controls stay their own.
- **ItemRow:** one vault item: tile, title (row title), non-secret subtitle, and small clock and key marks for a one-time code and a passkey. The whole row is one button; the tile is decoration; the marks read "One-time code" and "Passkey". Only the title and subtitle (user data) may be cut with an ellipsis. **ItemTile:** a serif initial (a whole grapheme, upper-cased when that keeps its length) in brass on forest, or the kind's glyph; a secure note's glyph sits on the brass wash; a blank title shows the key. `ItemRow` takes a `tileModifier`; the tapped row's tile and title travel into the item header.

### Inputs
- **HavenTextField:** the row is the field: no box, no border of its own. A muted label above the 16sp value, an optional muted placeholder; on focus the row takes the selection wash and a 1.5dp brass border; an error turns the label ember and adds an ember line below. Brass cursor and handles; the keyboard is asked not to learn. TalkBack reads its label and typed text as one node (the label is part of the field, not a second stop) and reports the error as the field's error. `HavenTextField` and `SecretTextField` take a `hint`: a muted line under the value, read with the field; an error takes its place.
- **SecretTextField:** the same row in mono, fully masked (no last-character flash) until the eye reveals it; the caller owns the revealed state so a lock resets it. Password keyboard, no autocorrect, no learning, no cut or copy. TalkBack reads it as a password field named by its label and the eye as "Show Password" / "Hide Password", never the value. An optional `placeholder` (a format, such as the Secret Key's `H1-XXXX-XXXX-…`) is drawn muted in mono in the empty value line and is not read; a `hint` is a line under the value that is read.
- **HavenSwitch / ToggleRow:** a 44 x 26dp track, line-strong off and brass on, a paper-white thumb that slides on the press spring, in a 52 x 48dp target. In a `ToggleRow` the whole row is the switch, named by its title, with press feedback; standalone it needs a label. TalkBack: name, "Switch", on/off.
- **HavenSlider:** brass fill on a 4dp line-strong track with a 24dp paper-white thumb; tap or drag anywhere on its 48dp height, snapped to steps. It draws no visible label or value; TalkBack gets the label and `valueText` ("24", not a percentage) and adjusts it with its own gestures.
- **SegmentedControl:** a field track (hover green in light) with the chosen segment raised on a hairline and a small shadow, the thumb fading between segments. Each segment is a 48dp tab; TalkBack reads it as a tab, selected or not, in a selectable group. Segments share the tallest one's height when a label wraps.

### Actions
- **HavenButton:** 48dp minimum, 10dp radius, 16sp 550 label that wraps and is never cut, optional 18dp glyph. Primary (brass in dark, forest in light; one per screen), secondary (outlined line-strong), quiet (bare text), danger (ember). Press scales and washes; disabled fades to 42%. TalkBack: the label, "Button", disabled when so.
- **HavenIconButton:** a 22dp glyph in a 48dp round target, muted unless tinted (ember for delete). Its content description is its name.
- **CopyButton:** a 48dp round target whose copy glyph cross-fades to a green check for 1.5 s, with a confirm haptic. The caller copies, toasts and clears the clipboard. TalkBack reads "Copy Password" (the field's name, never its value) and then "Copied" as its state.
- **Pill:** a 12sp marker: brass wash with brass ink for a state or device ("This device"), or outlined and muted for how something applies ("Whole site"). Not a control.
- **AddButton:** the floating 56dp disc in the primary colour with a 26dp plus and a 10dp shadow, placed by `HavenScaffold`. TalkBack: "New item".

### Overlays
- **HavenSheet:** a bottom sheet in its own secure window: raised ground, 18dp top corners, a 36 x 5dp handle, an optional serif title (a heading, and the window's pane title). It rises on the sheet spring once measured, the scrim fades with it; drag down past 40% of its height or tap the backdrop (labelled "Close") to close. A close tapped while it rises goes straight to closing; `onDismiss` is called once, after it has gone. An optional `footer` (the sheet's answers, such as Save and Cancel) is drawn under the content outside its scroll, so a sheet capped at 90% of a small window keeps its answers in view. It stops at 90% of the window and scrolls inside when taller (a short sheet still drags down from anywhere), and it rises above the keyboard.
- **HavenDialog:** a question in its own secure window: raised, 18dp corners, up to 360dp, 24dp padding, serif title (heading and pane title), body message, quiet dismiss and primary or danger confirm at the end. The first tap answers; the buttons then disable. The window dims by the scrim's alpha. It takes an `alternative` (the three answers stack full width) and `dismissible = false` (only a button answers).
- **HavenMenu:** a small menu below its trigger, end-aligned (above it when there is no room), in a focusable secure window that Back and an outside tap close. Raised, 14dp corners, line-strong border; 48dp rows with a muted glyph and 15sp label, ember for danger. It fades and scales in from its top end. Picking a row closes the menu, then acts.
- **ChoiceSheet / ChoiceRow:** single choices as radio rows with a brass check; a pick closes the sheet, then saves.
- **Toast:** near-opaque dark glass in both themes (the phone cannot blur behind it), a fully round pill with a hairline rim (`glass-rim`) and a 16dp shadow: a green check or ember alert and one line of body text. A queue of one; a new toast replaces the current one; it stays 2.5 s, rises on the toast spring and fades out. It is a polite live region (assertive for an alert), so TalkBack announces it, and it stays as long as the system's accessibility timeout recommends. It never carries a secret ("Password copied · clears in 30 s").

### Feedback
- **ProgressRing:** a 28dp ring with a 3dp round stroke on a line track: given progress it fills or drains on the tick spec (the one-time code), brass or ember when warning; without it a 270° arc spins, and under "Remove animations" it stands still. TalkBack gets a determinate or indeterminate progress range and an optional description.
- **PullToRefresh:** pulling past the top of a list moves the content at half the finger's speed; past 72dp a release refreshes once and the ring rests at 54dp spinning until done, then everything settles back on the smooth spring. TalkBack users get a "Refresh" custom action on the container instead of the pull; the ring reads "Refreshing".

### Text and plumbing
- **HavenText:** the kit's text, plain or annotated (a password with coloured digits and symbols); colour defaults to the provided content colour, then `text`. It wraps and is never cut unless the caller passes an ellipsis for user data.
- **HavenIcon / IconGlyph:** the desktop's icon set path for path, plus home, items, chevronRight, chevronLeft and more drawn to the same rules (and added to the desktop set; `HavenIconTest` keeps them identical). With a description it is an image TalkBack reads; with null it is decoration.
- **HavenPress / havenClickable:** the kit's only press feedback and its clickable (role button by default, no ripple) `HavenTheme` provides `HavenPress` as the default indication, so a clickable that names none still presses this way.
- **NoPersonalizedLearning:** asks the keyboard not to learn from any field inside it (`IME_FLAG_NO_PERSONALIZED_LEARNING`).
- **KitPreview:** the preview frame (theme on the pane, 16dp padding); previews and the debug catalogue only.

The debug build's `KitCatalogueActivity` (`app/src/debug/.../catalogue/`) shows every kit component in a picked theme (System, Light, Dark), grouped as Foundations, Actions, Structure, Inputs, Overlays and Feedback; `CatalogueTest` checks every kit file appears.

## Named Rules

**The No Ripple Rule.** Nothing ripples. Every press is `HavenPress`: a 0.97 scale and a brass-soft wash on the press spring, cut under "Remove animations". Put it after the clip and before the background so the ground scales with the content.

**The One Fitting Rule.** Brass marks controls and thin lines, never large surfaces; one primary per screen, brass in dark and forest in light; the add button counts as that primary.

**The Row Is the Field Rule.** A field never gets a box. The grouped row is the field; focus marks the row with the selection wash and a brass border, and the label above names it.

**The Secure Windows Rule.** Every overlay that opens a window of its own (sheet, dialog, menu) sets FLAG_SECURE, excludes itself from autofill and drops taps that pass through another app's window (`SecureDialogWindow`). The activity's protections do not reach these windows, so each states them.

**The Words Wrap Rule.** Our own words wrap and are never cut, at any font size: button labels, titles, messages, menu rows. Only user data (titles, usernames, URLs) may end in an ellipsis.

**The Foundation Only Rule.** Everything builds on `androidx.compose.foundation`; HavenKeys has no Material dependency (removed in stage 5). `forbidMaterial`, which `detekt` runs first, fails the build on any mention of `androidx.compose.material`, `androidx.compose.material3` or `com.google.android.material` in any Kotlin source (tests included) or in the version catalog, and detekt's `ForbiddenImport` refuses the same imports. The debug build still carries `material3` at runtime because `ui-tooling` depends on it; no source can compile against it.

**The Floating-Only Shadow Rule.** Only sheet, dialog, menu, toast and add button cast elevation shadows.

**The Stillness Rule.** "Remove animations" cuts everything, the spinner included.

## Do's and Don'ts

### Do:
- **Do** build details, forms and settings from `InsetGroup` rows, with hairlines starting at the text of the row below.
- **Do** give every control a 48dp target, and size text in sp.
- **Do** keep one primary per screen: brass in dark, forest in light; count the add button as it.
- **Do** set titles in HavenKeys Serif (32 unlock, 28 item and list, 22 sheet and dialog) and secrets and codes in JetBrains Mono.
- **Do** use the phone's contrast values (light muted `#606f69`, light brass ink `#7d632f`, dark on-danger `#121a17`) and keep `ContrastTest` passing.
- **Do** move on the named springs and the 160ms fade through `HavenTheme.motion`, so "Remove animations" cuts them.
- **Do** open sheets, dialogs and menus through the kit, so their windows are secure.
- **Do** name copy and reveal buttons by the field ("Copy Password"), and say what happened in a toast, never the value.
- **Do** rebuild fonts only with `scripts/build-android-fonts.py` and commit its output.

### Don't:
- **Don't** add a ripple, a Material component or a Material icon to the kit.
- **Don't** give a field its own box or border inside a row.
- **Don't** fill large surfaces with brass, or put brass ink text on light grounds at the desktop's `#8f7236`.
- **Don't** cut our own words with an ellipsis or a line limit.
- **Don't** put a secret in a toast, a content description or a label.
- **Don't** give groups or rows a shadow.
- **Don't** set labels, headings or counts in mono, or put all-caps tracked labels above headings.
- **Don't** download fonts or let a system face stand in for the serif.
- **Don't** turn dynamic colour on.

## Shell

The unlocked app is one frame (`ui/shell`, spec §6): a fixed top bar, the current tab's content and a bottom bar, with item screens, editors, the generator, devices and autofill setup over it full screen.

- **Top bar** (`ShellTopBar`): the search pill (field green in dark, hover green in light; an 18dp muted glyph, "Search HavenKeys" in muted value type, 12dp padding, an 8dp glyph gap), then the offline badge (a `Pill`, 4dp after the pill) when offline, Sync now (a 48dp icon button; a 20dp ring while syncing, a toast if it fails) and Lock. It sits outside the tabs' NavHost and takes only stable values, so a tab change neither moves nor recomposes it. Tapping the pill grows it into search's field (shared bounds).
- **Bottom bar** (`BottomBar`): Home, Items, Settings on the pane under a hairline, 60dp tall; each tab a 24dp glyph over a label, muted, the selected one in strong ink with an 18 x 2dp brass marker under it; a light tick on a change, none on a reselect.
- **Add sheet** (`AddSheet`): a kit `HavenSheet` titled "New item" with a two-column grid of 96dp tiles (12dp group radius, group ground, group hairline, a 24dp brass-ink glyph over the label) that settle in sequence: Login, Secure note, Card, Generate password. Offline the item tiles fade to 42% and a muted line says "Adding needs a connection"; the generator stays.
- **Lazy inset groups** (`insetGroup`, `InsetSlice`): the kit's `InsetGroup` cut into one lazy item per row, so long lists stay lazy; each slice draws its part of the 12dp outline and the hairline above it, starting where its text starts.
- **Lists**: Home has no large title (the identity card, then Recently added and Frequently used); Items, a category list and Settings open on a 28sp serif large title; a category list's back chevron sits above its title with its ink on the title's start.

## Full-screen screens

The screens over the shell (item, editor, generator, devices, autofill setup) share a few pieces in `ui/components`.

- **ScreenBar:** Back, the "Offline, read-only" marker (a polite live region, as the shell's bar has it), Lock, then the screen's own actions. It has no title: the large serif title sits under it.
- **OfflineNote:** the muted line that says why an action is dimmed offline.
- **MaskedValue / RevealedValue:** a hidden value is the fixed 12 dots, read as "Hidden <label>", never its length; a revealed one is mono with coloured digits and symbols (`style` sets the size, the generator's plate uses 20/30). Revealed values live only in composable state and clear after 30 s, on leaving the screen, when the app stops and on lock.
- **Item header:** a 56dp tile and the title in the headline style; the tapped row's tile and title travel into it.
- **Typed secrets** use `remember { TextFieldState() }`, never `rememberTextFieldState()` (which saves its text); they are never put in a route, `SavedStateHandle`, `rememberSaveable` or a view model.

## Review notes (stage 2)

**How the review ran.** No emulator or adb on the build machine. The catalogue was rendered with Robolectric native graphics (`CatalogueScreenshots` in `app/src/testDebug/.../catalogue/`), one PNG per section per theme plus the scaffold frame with a live toast, into `apps/android/.impeccable/review/kit-<light|dark>-<section>.png`, only when Gradle gets `-PscreensDir=.impeccable/review` (`./gradlew testGithubDebugUnitTest --tests '*CatalogueScreenshots*' -PscreensDir=.impeccable/review`). `captureToImage()` times out under Robolectric, so the test draws the decor view into a bitmap. Text renders correctly. Robolectric draws the window directly, so elevation shadows (`Modifier.shadow`) are not in the screenshots: the sheet, dialog, menu, toast and add button appear without them, and the light sheet and dialog drawn inline look edgeless (white on white); Paint shadow layers (thumbs, segmented thumb) do render. The `/impeccable` critique ran in a single context; its detector does not scan Kotlin, so the evidence was the screenshots, the kit sources and the desktop references.

**Verdict.** Recognisably the desktop's Vault Room adapted to touch, and nothing reads as Material. Type sizes match spec §5.1; every control is at least 48dp.

**Fixed in the review** (screenshots before: ef9153e; after: cec9d7e):
1. [P1] The dark toast vanished into the dark ground: the glass now draws the desktop's 1dp rim in both themes (e816f9c).
2. [P1] Hairlines started under the tile or glyph: each row now reports where its text starts and the hairline follows, mirrored in RTL (39ddcd8; `StructureTest.eachHairlineStartsWhereTheTextOfTheRowBelowItStarts`).
3. [P2] Switch and slider thumbs had a hard 1dp offset disc for a shadow: now the desktop's soft shadow via a Paint shadow layer (57bc661).
4. [P2] The dialog dimmed with the platform's 60% black: it now dims by the scrim's alpha (952158c; `OverlayWindowsTest`).
5. [P3] A trailing value sat 8dp from the row's end: `TrailingText` now sits on the 16dp margin (2860878; `StructureTest.aTrailingValueKeepsTheSameMarginAsTheRowsText`).
6. [P3] The chosen segment lacked the desktop's small shadow: added (623dc15).

**Checked and kept.** The new glyphs follow the desktop's drawing rules ("more" reads small at 22dp, as `grip` does on the desktop). Disabled controls fade to 42%; the dark disabled primary reads olive, as on the desktop. The dark segmented track is faint, as on the desktop; the raised thumb carries the selection. The dark add button is a brass disc because it is the screen's one primary. Menu radius is 14dp (desktop popover 11px) because the spec fixes the phone radii at 8/10/12/14/18.

**Decided.**
- Serif titles are kept: the desktop does it and the review recommends it (spec §5.1 listed the serif for monograms only).
- Toast glass: the phone cannot blur behind the toast, so its glass is 95% opaque (0xF2) in both themes, near-opaque, with the hairline rim.
- GroupRow disclosure chevron for navigating rows with trailing content (the Items tab's counts): decided in stage 3.

**Open for the owner.**
- Should the desktop adopt the phone's contrast values? Its own fail WCAG AA: muted 4.45:1, light brass ink 4.17:1, white on ember 3.01:1. The review recommends all three, in `packages/ui` tokens.
- The slider's visible value label: `HavenSlider` shows none, so the generator screen must show "Length 24" itself unless the kit takes that line on.

**Device checks not yet run** (no emulator on the build machine; for the owner, in the debug catalogue `net.havenkeys.android/.catalogue.KitCatalogueActivity`):
- Emulator or device screenshots, including the elevation shadows of sheet, dialog, menu, toast and add button.
- Animations by hand at animator scale 1 and 0: the sheet spring, drag down and backdrop tap close it, a tap during the rise closes it once; dialog, menu (scales in from its top end), toast (rises), copy glyph to check, switch, segmented control, pull to refresh. At 0 each cuts with nothing blocked. Judge the springs' feel.
- The full TalkBack pass: each control's name, role and state once (button, switch on/off, tab selected, slider "24"); a text field reads its label once, and whether a label set as `contentDescription` hides the typed value of a non-secret field; the secret field reads as a password and never speaks its value; the copy button says "Copy Password" then "Copied"; the toast is announced; sheet, dialog and menu titles are announced; an item row is one stop (title, subtitle, "One-time code", "Passkey"); the pull-to-refresh "Refresh" action is reachable, and if not, Home needs a visible sync control. Confirm TalkBack skips the hairlines.
- FLAG_SECURE on the sheet, dialog and menu windows: a screenshot of them comes out black.

## Review notes (stage 3)

**How the review ran.** As in stage 2: `ShellScreenshots` (`app/src/testDebug/.../screens/`) renders with fake repositories (no vault data) the shell on Home, Items, a category list and Settings, Home with a failed load, search empty and with results, and the add sheet offline, in light and dark at 411dp, plus the shell at 360dp in English and Portuguese, into `apps/android/.impeccable/review/shell-*.png` (git-ignored), only with `-PscreensDir=.impeccable/review`. Shadows and the status-bar inset are not in the images (Robolectric draws the window directly); the sheet is drawn in place because it opens its own window. The `/impeccable` critique ran in a single context; its detector does not scan Kotlin.

**Verdict.** One product with the desktop and the stage 2 kit; nothing reads as Material. Home's rhythm (identity card, two titled groups) and the Items and Settings groupings hold; the One Fitting Rule holds (brass on the switch, the marker, the add button, tile glyphs and "Clear"; one primary, the add button).

**Fixed in the review:**
1. [P1] In light theme the search pill had no ground (the field colour is white like the pane), so the bar read as loose text; it now takes the hover green, as the segmented track does, in the bar and in search (9e6bed8; `ShellPartsTest.theSearchPillStandsOffThePaneInBothThemes`).
2. [P1] Home showed "New items you add appear here." and "Items you fill or copy will show here." under an error line, claiming empty lists it could not load; the empty lines now show only without an error, as the category list does (7c93078; `HomeScreenTest.aFailedLoadSaysSoAndClaimsNoEmptyList`).
3. [P2] On a 360dp phone offline the pill's "Search HavenKeys" wrapped to two lines and the bar grew; tighter pill padding (14 to 12dp), glyph gap (10 to 8dp) and badge gap (8 to 4dp) keep it on one line in English; search's field matches so the shared-bounds move does not jump (bfdb03a; `ShellTopBarNarrowTest`).
4. [P3] The category list's back chevron sat 13dp right of the large title (the glyph centred in its 48dp target on the gutter); it is now pulled out by that inset so its ink lines up with the title (765bfbd; `ItemsScreensTest.theBackChevronsGlyphLinesUpWithTheLargeTitle`).
5. [P1] Light muted on the pill's hover ground was 4.47:1; light muted darkens from `#61706a` to `#606f69` (4.53:1 there, 4.86:1 on a group), and `ContrastTest` now checks text on the search pill and segmented track ground (94fb8c4).
6. [P2] In Portuguese the placeholder is "Buscar" (was "Buscar no HavenKeys", which wrapped at 360dp offline); English is unchanged (`ShellTopBarNarrowTest.inPortugueseToo`).

**Checked and kept.** Home without a large title (1Password's Home has none; the identity card leads). The add sheet's dimmed tiles keep their brass-ink glyphs at 42%. Dark item tiles (avatar green on group green) are quiet, as on the desktop; the brass initial carries them. Bottom-bar labels fit at 360dp in Portuguese ("Configurações").

**Open for the owner.**
- At large font sizes the pill's placeholder still wraps on a narrow phone offline (the words wrap, so nothing is cut, but the bar grows). If that matters: while offline, the offline badge could take the place of the disabled Sync now, which changes the plan's top bar.

**Decided.**
- Sync now is disabled while offline (dimmed, read as disabled by TalkBack) and announced while syncing; the device checklist matches.
- The identity card's summary may wrap under the Words Wrap Rule; spec §6.5's "one-line summary" is read as "one summary".

## Review notes (stage 4)

**How the review ran.** As in stages 2 and 3: `ScreenScreenshots` (`app/src/testDebug/.../screens/`) renders with fake repositories (no vault data) item detail masked and with the password revealed and a live code, the editor for a login, a secure note, a card and the identity, the generator, unlock with the Secret Key and biometrics, onboarding (choose, type the kit, password), devices, autofill setup offline, Search HavenKeys with a result, the card and identity confirmation with three answers and the passkey save sheet, in light and dark at 411dp, plus the editor and the passkey sheet on a 360 x 640dp phone, into `apps/android/.impeccable/review/screens-*.png` (git-ignored), only with `-PscreensDir=.impeccable/review`. Sheets and dialogs are drawn inline. The `/impeccable` critique ran in a single context (no subagents, by the controller's rule); its detector does not scan Kotlin (it returned no findings).

**Verdict.** The rebuilt screens and the stage 3 shell read as one app, and as the desktop's Vault Room: serif titles over inset groups of hairline rows, mono only for secrets and codes, one primary per screen (Save in the editor's bar, Copy in the generator, Unlock, Choose HavenKeys for autofill, Fill with documents, Save on the passkey sheet). Nothing reads as Material. Brass stays on controls, thin lines, the code ring, the lock glyph and ink.

**Fixed in the review:**
1. [P1] The passkey sheet's Save and Cancel scrolled below the fold when the logins filled a small phone; the kit's `HavenSheet` gained a `footer` drawn outside the scroll, and the sheet pins its answers there (b7f24a5; `PasskeyCreateScreenTest.saveAndCancelStayInViewWhileTheLoginsScroll`, `OverlaysTest.aTallSheetsFooterStaysInViewUnderItsScrollingContent`).
2. [P2] TalkBack read the generator's length twice ("Length, 24" from the row, again from the slider); the visible row is hidden from TalkBack and the slider speaks label and value (36871c5; `GeneratorScreenTest.theLengthIsReadOnceByTheSliderAlone`).
3. [P2] The generator's plate opened with the lede and showed the password at field size; as on the desktop the lede sits under the title and the plate leads with the password in mono 20 on 30 (f27f9e0). `RevealedValue` takes a `style`.
4. [P1] Unlock showed the Secret Key's format as a hint line under an empty value line, so it read as a misplaced value; `SecretTextField` gained a `placeholder`, drawn muted in the empty field and not read (5d3e1c8; `TextFieldsTest.aSecretFieldsPlaceholderShowsOnlyWhileEmptyAndIsNotRead`).
5. [P3] Devices said "Last seen never"; it says "Not seen yet" ("Ainda não visto") (9b9f623).
6. [P2] Autofill setup showed the autofill state as a row with a glyph but the passkeys state as a loose paragraph in a different ink; both are now the same state row (c24bd46).
7. [P3] The editor's Save ended 12dp from the edge, 4dp outside the fields' 16dp gutter; it now ends on it (2d51fb5; `EditScreenTest.saveEndsOnTheSameGutterAsTheFields`).
8. [P2] Unlock's headline had lost the desktop's one italic word: "locked" is set in the serif italic in brass ink (this file's Display rule) (8cb01f1; `UnlockScreenTest.theHeadlineSetsLockedInItalic`). Brass ink on the pane is already in `ContrastTest`; no new colour pairing was added.

**Checked and kept.** Item detail: tile and serif title, then one group of rows; the masked row keeps the fixed 12 dots; the revealed password colours digits and symbols; the code reads in two halves with its ring. The editor's two quiet buttons (Change, Remove) on a hidden row fit at 360dp; "Set up." for a present one-time code is the desktop's wording. The website group's hairline under "Matches" starts at the Add website row's text, by the hairline rule. The confirmation's three stacked answers (primary, secondary, quiet Cancel) read in order of weight. The light disabled primary reads grey (kit decision, stage 2).

**Open for the owner.**
- Onboarding's choose step titles its group "How to set up" in the small serif title, where other screens use the muted sans section header. Keep it as a lead line, or make it a `SectionHeader`?
- Search HavenKeys (autofill) shows only the search pill and results: nothing on screen names the app being filled until the confirmation ("Use <login> in <app>?"). A muted line under the pill ("Filling in Example") would orient the user; it adds copy the spec does not have.
- Devices: the desktop puts "This computer" inline after the name; the phone puts "This phone" above the name. Either works; the inline form saves a line.
- Devices offers Revoke on this phone's own row (pre-existing). Should this phone's row offer Sign out instead, or nothing?
- An empty hidden field in the editor (a passport not yet set) shows the eye with nothing to reveal; it could hide the eye until something is typed (kit change).

**Device checks not yet run** (for the owner's checklist in `docs/android.md`): the passkey sheet on a real small phone with many logins and a large font (answers stay in view above the navigation bar and the keyboard); TalkBack on the generator (length read once, "24"); unlock's italic word at large font sizes (it wraps with the line).

## Review notes (stage 5)

**How the review ran.** The three screenshot tests (`CatalogueScreenshots`, `ShellScreenshots`, `ScreenScreenshots`) rendered before and after the removal into `apps/android/.impeccable/review/stage5-before` and `stage5-after` (git-ignored) with `./gradlew testGithubDebugUnitTest --tests '*Screenshots*' -PscreensDir=...`; the files compared byte for byte: 72 of 72 identical after the removal itself. `/impeccable` critiqued the after set in a single context (no subagents, by the controller's rule; its interview was skipped because the owner was away); its detector does not scan Kotlin. After the three fixes below the set was rendered again: exactly the 8 files for unlock and the three onboarding steps (light and dark) differ from `stage5-before`, the other 64 are identical.

**What left.** `ui/theme/MaterialBridge.kt` (the bridge, both Material colour schemes, `MaterialTypography`, the unused legacy `HavenType`, `MaterialShapes`), `compose-material3` and `compose-icons` (`material-icons-extended`), and the six `material-icons-*` verification entries (none had to be restored). `compose-foundation` is now an explicit dependency instead of arriving through `material3`.

**Release APK** (unsigned `githubRelease`, same Rust libraries both times). Before: commit 61bf35b. After: commit a6d074a.

| | Before | After | Difference |
|---|---|---|---|
| Total APK | 75,257,943 | 75,197,588 | -60,355 B (-58.9 KiB) |
| `classes*.dex` | 4,213,576 | 4,181,768 | -31,808 B (-31.1 KiB) |
| `resources.arsc` | 480,496 | 457,488 | -23,008 B (-22.5 KiB) |

Nothing grew. The saving is small because R8 had already stripped most of Material; the APK is dominated by the Rust libraries.

**Verdict.** The app reads as one product with no Material underneath: a serif large title over inset groups of hairline rows, a 12dp group radius, muted sans section headers, mono only for secrets and codes, 48dp targets. The One Fitting Rule holds across all screens at once, each screen has one primary, and nothing reads as Material (no ripple, sheet, type scale, icon or text-field box). Contrast in both themes is held by `ContrastTest`; no colour pairing was added. Heuristic score (single context, Operate surface): 31/40, Good.

**Fixed in the review:**
1. [P2] Unlock sat on a 20dp gutter left over from M1; it uses `HavenSpacing.gutter` (16dp) like every other screen (25c89b5; `UnlockScreenTest.unlockSitsOnTheSameGutterAsEveryOtherScreen`).
2. [P3] Onboarding's back slot was 48dp where `ScreenBar` is 56dp, so its title sat 8dp higher than every other full-screen large title; the slot pads 4dp vertically (b3fc2d7; `OnboardingScreenTest.theLargeTitleSitsWhereItDoesUnderTheScreenBar`).
3. [P3] Onboarding's kit-password lede was in body ink; it is muted, as unlock's and the generator's are (a6d074a; no behaviour, so no test).

**Checked and kept.** Light item tiles are forest with a brass initial (Avatar is that in both themes, as on the desktop). The Secret Key placeholder's ending "..." belongs to the format hint `H1-XXXX-XXXX-...`. Onboarding's typed-kit fields keep their supporting hints under an empty value line (see the open question). The add sheet's dimmed tiles, the search Cancel, the passkey sheet's pinned answers and the generator's brass-ink strength line stay.

**Decided.**
- The kit's press is the default indication: `HavenTheme` provides `HavenPress` as `LocalIndication`, so no clickable can ripple, and `KitTextInput` provides the brass text-selection colours for every text field, search's included.
- The debug build's runtime carries `material3` through `ui-tooling`; that is accepted, because no source can compile against it.
- Two guards: `forbidMaterial` (Gradle, every source and the catalog, no exclusions) and detekt's `ForbiddenImport`.
- The `androidx.collection:collection:1.6.0` constraint in `app/build.gradle.kts` exists only to avoid adding a new verification hash; drop it when a future verified bump makes it the natural resolution.

**Open for the owner.**
- Onboarding "How to set up": small serif title or `SectionHeader`? Seen with the whole app, the app has three section-heading forms (that one, `SectionHeader`, and strong `groupTitle` over prose in autofill setup). Recommendation: `SectionHeader` for "How to set up"; the autofill setup headings head prose and can stay.
- Search HavenKeys names no app until the confirmation; a muted "Filling in <app>" line is the cheapest fix but adds copy the spec does not have.
- Devices "This phone": the pill above the name costs a line and is the only pill-over-title row; the desktop's inline form would read more like the other rows.
- Revoke on this phone's own row is one tap from revoking the device in hand (the most consequential question). Recommendation: Sign out, or nothing, on that row.
- An empty hidden field (Passport in the identity editor) shows the eye with nothing to reveal (kit change).
- Onboarding's typed-kit step puts supporting hints under empty value lines (Server, Secret Key) while Email and unlock's Secret Key use placeholders. Should Secret Key take unlock's `H1-XXXX-XXXX-...` placeholder so the same field looks the same in both places?
- The TalkBack pass over the whole app is a device check (no emulator); it is on the stage 5 checklist in `docs/android.md`.

**Device checks not yet run.** Everything in the stage 2 to 4 lists, plus the stage 5 list in `docs/android.md`: brass selection handles and wash in an editor field and the search pill, and the kit press (no ripple) on a long-press of any row.
