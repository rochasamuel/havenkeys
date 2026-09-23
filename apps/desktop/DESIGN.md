---
name: HavenKeys (desktop app and browser extension)
description: A calm, native-feeling vault in Apple's three-pane grammar, wearing the website's forest green, brass fitting and serif titles; the extension is the same room in miniature.
colors:
  pane: "#0f1614"
  sidebar: "#0b110f"
  sidebar-light: "#16231f"
  list: "#121b18"
  raised: "#172320"
  group: "#152120"
  group-light: "#f3f6f4"
  group-line: "rgba(226, 234, 230, 0.07)"
  group-line-light: "#e2e8e5"
  field: "#0b110f"
  hover: "#1b2925"
  line: "#1f2c28"
  line-strong: "#2c3d37"
  text: "#e2eae6"
  text-strong: "#f3f7f5"
  muted: "#86968f"
  pane-light: "#ffffff"
  list-light: "#f4f7f5"
  text-light: "#1e2b27"
  text-strong-light: "#121a17"
  muted-light: "#66756f"
  brass: "#c9a45c"
  brass-hi: "#e3c483"
  brass-ink-light: "#8f7236"
  brass-soft: "rgba(201, 164, 92, 0.14)"
  sel: "rgba(201, 164, 92, 0.15)"
  on-brass: "#121a17"
  primary-light: "#16231f"
  primary-hover-light: "#2b3f39"
  primary-fg-light: "#f1f5f3"
  avatar: "#1d2b27"
  side-text: "#c3cfc9"
  side-muted: "#7f8f88"
  side-hover: "#16211e"
  glass: "rgba(15, 22, 20, 0.9)"
  glass-light: "rgba(22, 35, 31, 0.94)"
  symbol: "#8fc4ad"
  ok: "#5fa785"
  danger: "#e0775f"
  danger-light: "#b4412f"
  paper-sheet: "#fbfcfb"
  paper-text: "#3a4842"
  paper-strong: "#121a17"
  paper-muted: "#5a6a63"
  paper-well: "#edf1ee"
  paper-line: "#c7d2cc"
typography:
  display:
    fontFamily: "Source Serif 4 Variable, Source Serif 4, Georgia, serif"
    fontSize: "30px"
    fontWeight: 460
    lineHeight: 1.1
    letterSpacing: "-0.015em"
  emphasis:
    fontFamily: "Source Serif 4 Variable, Source Serif 4, Georgia, serif"
    fontWeight: 400
    fontFeature: "italic"
  headline:
    fontFamily: "Source Serif 4 Variable, Source Serif 4, Georgia, serif"
    fontSize: "28px"
    fontWeight: 480
    lineHeight: 1.15
    letterSpacing: "-0.015em"
  title:
    fontFamily: "Source Serif 4 Variable, Source Serif 4, Georgia, serif"
    fontSize: "22px"
    fontWeight: 480
    letterSpacing: "-0.012em"
  title-sm:
    fontFamily: "Source Serif 4 Variable, Source Serif 4, Georgia, serif"
    fontSize: "17px"
    fontWeight: 500
  body:
    fontFamily: "Hanken Grotesk Variable, Hanken Grotesk, system-ui, sans-serif"
    fontSize: "13.5px"
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "\"ss01\", \"cv11\""
  value:
    fontFamily: "Hanken Grotesk Variable, Hanken Grotesk, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
  row-title:
    fontFamily: "Hanken Grotesk Variable, Hanken Grotesk, system-ui, sans-serif"
    fontSize: "13.5px"
    fontWeight: 600
  label:
    fontFamily: "Hanken Grotesk Variable, Hanken Grotesk, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 560
  group-title:
    fontFamily: "Hanken Grotesk Variable, Hanken Grotesk, system-ui, sans-serif"
    fontSize: "12.5px"
    fontWeight: 600
  mono:
    fontFamily: "JetBrains Mono, ui-monospace, monospace"
    fontSize: "14px"
    fontWeight: 500
  code:
    fontFamily: "JetBrains Mono, ui-monospace, monospace"
    fontSize: "21px"
    fontWeight: 500
    letterSpacing: "0.05em"
rounded:
  hairline: "2px"
  key: "5px"
  control-sm: "7px"
  control: "8px"
  row: "9px"
  menu: "10px"
  popover: "11px"
  group: "12px"
  output: "14px"
  avatar-lg: "15px"
  sheet: "6px"
  pill: "999px"
spacing:
  row-y: "9px"
  row-x: "16px"
  group-gap: "14px"
  group-title-top: "24px"
  pane-x: "36px"
  pane-top: "26px"
  sidebar: "232px"
  list: "300px"
  detail-max: "720px"
components:
  button-primary:
    backgroundColor: "{colors.brass}"
    textColor: "{colors.on-brass}"
    rounded: "{rounded.control}"
    padding: "0 13px"
    height: "32px"
  button-primary-hover:
    backgroundColor: "{colors.brass-hi}"
  button-primary-light:
    backgroundColor: "{colors.primary-light}"
    textColor: "{colors.primary-fg-light}"
    rounded: "{rounded.control}"
    height: "32px"
  button-primary-light-hover:
    backgroundColor: "{colors.primary-hover-light}"
  button-brass:
    backgroundColor: "{colors.brass}"
    textColor: "{colors.on-brass}"
    rounded: "{rounded.control}"
    padding: "0 13px"
    height: "32px"
  button-secondary:
    backgroundColor: "transparent"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.control}"
    padding: "0 13px"
    height: "32px"
  button-secondary-hover:
    backgroundColor: "{colors.hover}"
  button-small:
    rounded: "{rounded.control-sm}"
    padding: "0 10px"
    height: "28px"
  button-large:
    rounded: "{rounded.menu}"
    height: "42px"
    width: "100%"
  button-danger:
    backgroundColor: "{colors.danger}"
    textColor: "#ffffff"
    rounded: "{rounded.control}"
    height: "32px"
  icon-button:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.control-sm}"
    size: "30px"
  unlock-field:
    backgroundColor: "{colors.field}"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.group}"
    padding: "0 54px 0 16px"
    height: "48px"
  unlock-go:
    backgroundColor: "{colors.brass}"
    textColor: "{colors.on-brass}"
    rounded: "{rounded.row}"
    size: "34px"
  sidebar:
    backgroundColor: "{colors.sidebar}"
    textColor: "{colors.side-text}"
    width: "232px"
  nav-item:
    backgroundColor: "transparent"
    textColor: "{colors.side-text}"
    rounded: "{rounded.control-sm}"
    padding: "0 10px"
    height: "32px"
  nav-item-current:
    backgroundColor: "{colors.sel}"
    textColor: "{colors.text-strong}"
  list-item:
    backgroundColor: "transparent"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.row}"
    padding: "8px 10px"
  list-item-selected:
    backgroundColor: "{colors.sel}"
  avatar:
    backgroundColor: "{colors.avatar}"
    textColor: "{colors.brass-hi}"
    typography: "{typography.title-sm}"
    rounded: "{rounded.row}"
    size: "34px"
  group:
    backgroundColor: "{colors.group}"
    rounded: "{rounded.group}"
  row:
    textColor: "{colors.text-strong}"
    padding: "9px 12px 9px 16px"
    height: "50px"
  edit-row-focus:
    backgroundColor: "{colors.sel}"
  pill:
    backgroundColor: "{colors.brass-soft}"
    textColor: "{colors.brass-hi}"
    rounded: "{rounded.pill}"
    padding: "1px 8px"
  toast:
    backgroundColor: "{colors.glass}"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.pill}"
    padding: "10px 16px 10px 13px"
  kit-sheet:
    backgroundColor: "{colors.paper-sheet}"
    textColor: "{colors.paper-text}"
    rounded: "{rounded.sheet}"
    padding: "36px 40px 30px"
  segmented:
    backgroundColor: "{colors.field}"
    rounded: "{rounded.row}"
    padding: "2px"
  segmented-selected:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.control-sm}"
    padding: "5px 13px"
  ext-popup:
    backgroundColor: "{colors.pane}"
    textColor: "{colors.text}"
    width: "320px"
  ext-menu-card:
    backgroundColor: "{colors.pane}"
    textColor: "{colors.text}"
    rounded: "{rounded.menu}"
  ext-menu-row:
    backgroundColor: "transparent"
    rounded: "{rounded.control}"
    padding: "0 8px"
    height: "46px"
  ext-menu-row-hover:
    backgroundColor: "{colors.brass-soft}"
  ext-save-prompt:
    backgroundColor: "{colors.pane}"
    rounded: "{rounded.menu}"
    width: "340px"
    height: "138px"
  ext-button-primary:
    backgroundColor: "{colors.brass}"
    textColor: "{colors.on-brass}"
    rounded: "{rounded.control}"
    padding: "0 14px"
    height: "30px"
---

# Design System: HavenKeys (desktop app and browser extension)

## Overview

**Creative North Star: "The Vault Room"**

The desktop app is the room the website shows from outside: the same forest-green grounds and single brass fitting, arranged in the grammar of Apple's own apps. A source-list sidebar, an item list and a detail pane sit side by side; details and forms are inset groups of rows divided by hairlines, never a bordered card per field. Titles speak in Source Serif 4, everything else in Hanken Grotesk, and secrets alone in JetBrains Mono. The shield-and-keyhole mark is the only emblem; behind the unlock and welcome screens a faint brass guilloche rosette turns very slowly, the banknote pattern inherited from the website.

The browser extension is the same room in miniature: the popup is the vault list at 320px, the in-page menu and save prompt are small cards in the same grounds, and the options page is laid out like the app's Settings. Both surfaces take colour, radius and text tokens from `@havenkeys/ui` (the desktop via `tokens.css` and its `data-theme` switch; the extension via `tokens-auto.css`, which follows the OS colour scheme). The desktop adds a per-theme layer at the top of `src/styles.css` (pane, group, group-line, field, sel, sel-strong, glass, lift, serif) and a motion grammar.

Motion is quiet and physical, on one curve (`cubic-bezier(0.32, 0.72, 0, 1)`) at three speeds (160, 280, 420ms): on unlock the key in the mark turns and the vault rises in (scale 0.985 to 1 with a fade), panes cross-fade up 6px on selection, copy buttons morph into a check, a revealed secret un-blurs, and the TOTP ring drains continuously. Reduced motion turns all of it off.

**Key Characteristics:**
- Three panes (232px sidebar, 300px list, detail up to 720px); tools span list and detail at a centred 660px reading width.
- Inset groups (12px radius) of hairline-separated rows; the editor uses the same rows with borderless inputs.
- Serif titles; one brass italic, on the unlock headline only.
- Mono only for secrets, one-time codes, keys and the literal machine strings around them.
- Brass as a fitting: selection wash, the committing action, focus, the live TOTP code.
- The sidebar is forest green in every theme; the Emergency Kit sheet is paper in every theme.

## Colors

A forest-green ramp, a single brass fitting and one paper sheet; dark is the default, light swaps the panes to white and keeps the sidebar dark.

### Primary
- **Fitting Brass** (brass): the committing button in dark theme, the unlock arrow, focus borders, the TOTP ring, the switch track when on, the slider fill, the mark's shield and key. Always a control or a thin line, never a large surface.
- **Lit Brass / Brass Ink** (brass-hi, brass-ink-light): brass that carries text: the TOTP code, the unlock italic, the "Add website" row, pills, the selected nav icon. In light theme brass that carries text darkens to `brass-ink-light` or it disappears.
- **Selection Wash** (sel, and brass-soft): the selected list row, the current sidebar item, a focused editor row, menu hover, the secure-note avatar and the pill ground.

### Secondary
- **Cipher Green** (symbol): symbols inside a generated password; digits in that password take brass-ink so they read apart from letters.

### Tertiary
- **Leaf OK** (ok) and **Ember** (danger, danger-light): connected state, copy checks, strength bar; errors, the low TOTP ring, offline, destructive buttons.

### Neutral
- **Window Green** (pane): the detail pane, unlock and welcome grounds, the popup and in-page cards. White (`pane-light`) in light theme.
- **Sidebar Forest** (sidebar, sidebar-light): the source list. `#0b110f` in dark, `#16231f` in light: dark in every theme.
- **List Green** (list, list-light): the middle column.
- **Group Green** (group, group-light) with **Group Hairline** (group-line, group-line-light): inset groups and the rows inside them.
- **Field Green** (field): unlock and form inputs, segmented control track.
- **Raised / Hover** (raised, hover): menus, the segmented thumb, hover rows.
- **Hairline / Strong Hairline** (line, line-strong): pane dividers; the border of every interactive outline.
- **Bone / Bright Bone / Lichen** (text, text-strong, muted, and their light counterparts): body, titles and values, labels and meta.
- **Sidebar inks** (side-text, side-muted, side-hover): text and states on the sidebar, which never follow the theme.
- **Glass** (glass, glass-light): the toast, a dark translucent pill in both themes.
- **Paper set** (paper-sheet, paper-text, paper-strong, paper-muted, paper-well, paper-line): the Emergency Kit sheet only.

### Named Rules
**The One Fitting Rule.** Brass is a fitting, not a finish: selection, the one committing action, focus, and the live TOTP code. In light theme the everyday primary button turns forest (`primary-light`) and brass stays on the unlock arrow, the kit's Done and the fittings.

**The Evergreen Sidebar Rule.** The sidebar is forest green in dark, light and system themes. Its text uses the `side-*` inks, never the theme's text tokens.

**The Paper Sheet Rule.** The Emergency Kit sheet is paper in every theme, because it is what gets printed. It is the only light surface in the dark theme and uses only the paper set.

## Typography

**Display Font:** Source Serif 4 Variable (with Georgia, serif), optical sizing
**Body Font:** Hanken Grotesk Variable (with system-ui, sans-serif), stylistic sets ss01 and cv11
**Label/Mono Font:** JetBrains Mono 500 (with ui-monospace, monospace)

**Character:** A bookish serif names things; a warm, precise grotesque does the work; mono is evidence that a string is exact.

### Hierarchy
- **Display** (460, 30px, 1.1): the unlock headline. The options page's h1 matches it at 480.
- **Headline** (480, 28px, 1.15): item title, editor and tool heads, Emergency Kit screen head (welcome h1 at 27px).
- **Title** (480, 22px): the list pane title ("All items"); the kit sheet's heading at 22px 500; the empty-detail line at 21px.
- **Small title** (500 to 520, 16 to 17px): avatar monograms, empty-list line, popup notice.
- **Body** (400, 13.5px, 1.45): the app's reading size; the popup and in-page frames run at 13px; the options page at 14px.
- **Value** (14px, text-strong): a row's value; notes read at 14px / 1.6.
- **Row title** (600, 13.5px): the first line of a list row; second line 12.5px muted.
- **Label** (560, 12px, muted): the label above a row value. Group titles at 12.5px 600, sentence case.
- **Code** (mono 500, 21px, 0.05em, tabular): the TOTP code in the detail pane; 15px / 0.06em in the popup.

### Named Rules
**The One Italic Rule.** The serif italic in brass appears once in the app: "HavenKeys is *locked*." on the unlock screen (400, brass-ink). Every other title is roman, with no emphasis.

**The Literal String Rule.** JetBrains Mono is for secrets, one-time codes, keys and the literal machine strings around them (Secret Key, setup keys, the server URL). Never for labels, headings or numbers-as-decoration.

**The Sentence Case Rule.** Labels, group titles and sidebar headings are sentence case at their size; no all-caps tracked labels.

## Layout

The window is a three-column grid: 232px sidebar, 300px list, and a detail pane whose content is capped at 720px with 26px / 36px padding. Below 960px the columns tighten to 200px and 260px and pane padding drops to 22px. Tools (generator, settings) span the list and detail columns and centre themselves at 660px. The list head is 64px tall with the serif title on its baseline; the sidebar top row is 52px.

On macOS the window uses an overlay title bar and the root carries `data-platform="mac"`: the sidebar top row becomes 38px with a 76px left inset for the traffic lights. This is built and rendered but not yet verified on a real Mac.

Rhythm is dense and 2px-based: rows at 50px minimum (48px in the editor), groups 14px apart, group titles 24px above their group. Unlock and welcome centre a single column (380px and 400px) over a soft radial raise.

The extension popup is 320px wide with a 48px bar. The in-page frames are sized by `content/frames.ts`: header 34px, rows 46px, 8px of vertical padding (4px each side), save prompt 340 x 138px. The options page is a 640px column.

### Named Rules
**The Fixed Frame Rule.** In-page menu and save-prompt geometry is owned by `content/frames.ts` (header 34px, rows 46px, 8px padding, save prompt 340 x 138). CSS must agree with those numbers exactly; change both or neither.

## Elevation & Depth

Depth is tonal: the sidebar, list and pane step through greens, and groups sit on a slightly lifted surface with a hairline. Shadows are soft, offset downward and reserved for things that float: the new-item menu, the toast and the Emergency Kit sheet (lift), plus the small thumbs of the switch, slider and segmented control. Focus is a ring, not a shadow.

### Shadow Vocabulary
- **Lift** (`0 24px 48px -16px rgba(0,0,0,0.6), 0 8px 16px -8px rgba(0,0,0,0.45)`; light: `0 24px 48px -18px rgba(22,35,31,0.28), 0 8px 16px -8px rgba(22,35,31,0.14)`): popover menus, the toast, the kit sheet.
- **Thumb** (`0 1px 3px rgba(0,0,0,0.35)` to `0 1px 4px rgba(0,0,0,0.45)`): switch and slider thumbs on paper-white (#fbfcfb).
- **Segmented thumb** (`0 1px 3px rgba(0,0,0,0.25), inset 0 0 0 1px line-strong`): the selected segment.
- **Focus ring** (`0 0 0 2px var(--bg), 0 0 0 4px rgba(201,164,92,0.6)`): keyboard focus on buttons, switches and sliders. Inputs instead turn their border brass with a 3 to 4px brass-soft halo.

### Named Rules
**The Floating-Only Shadow Rule.** Only things that float over the window cast a shadow. Groups, rows, panes and the in-page cards stay flat with a hairline.

## Shapes

Gently rounded, sized to the object: 7px small controls and sidebar rows, 8px buttons and menu rows, 9px list rows and avatars, 10px in-page cards and large buttons, 11px popover menus, 12px inset groups and the unlock field, 14px the generator output, 15px the 60px detail avatar, 999px pills and the toast. The kit sheet is nearly square (6px). Borders are 1px; hairlines between rows are 1px group-line. Icons are one inline SVG family on a 24px grid with a 1.6 stroke and round caps.

## Components

### Buttons
- **Shape:** 8px radius, 32px tall; small 28px at 7px; large 42px full width at 10px.
- **Primary:** brass with on-brass text in dark; forest with pale text in light. One per view.
- **Brass:** always brass, in both themes (the Emergency Kit's Done).
- **Secondary:** transparent with a line-strong border; quiet and ghost drop the border; danger fills ember; quiet-danger is ember text with a 12% ember hover.
- **Hover / Active:** hover takes the hover ground (brass lifts to brass-hi); press scales to 0.97 (icon buttons 0.92). 160ms on the house ease. Disabled drops to 42% opacity.
- **Copy button:** the copy glyph cross-fades and scales into a green check, then back.

### Chips
- **Pill:** 999px, brass-soft ground, brass-ink text, 11px 600: device and state markers.
- **Match chip:** outlined pill (line-strong), 11.5px muted: how a URL matches ("Whole site").

### Cards / Containers
- **Inset group:** 12px radius, group ground, 1px group-line border, rows inside separated by group-line hairlines; overflow clipped so row highlights meet the corners.
- **Row:** 50px minimum, 9px 12px 9px 16px; a 12px muted label over a 14px value, trailing icon actions at 60% opacity that come to full on hover or focus.
- **Generator output:** 14px radius group surface, 22px padding, the password in mono 20px with brass digits and cipher-green symbols.

### Inputs / Fields
- **Editor rows:** the input has no border and no background; the row is the field. A 96px muted label sits left. On focus the row takes the selection wash and an inset 1.5px brass ring; the input's own focus ring is suppressed. The Notes area is the group itself, so its ring goes on the group.
- **Unlock field:** 48px, 12px radius, field ground, 15px with secret tracking; focus turns the border brass with a 4px brass-soft halo; invalid turns it ember and the fields shake. The brass arrow button sits inside at the right; the Secret Key field rises in beneath when needed.
- **Form fields (welcome, settings):** 36 to 40px, 9px radius, field ground; focus as above with a 3px halo.
- **Switch:** 38 x 22 track, line-strong off, brass on; white thumb that stretches while pressed.
- **Segmented:** field track, 9px radius; selected segment raised with a hairline and a small shadow.

### Navigation
The sidebar: brand row with the mark at 22px, a translucent search field (brass halo on focus), sentence-case section headings, 32px nav rows at 7px radius with muted icons and tabular counts. The current row takes the brass wash with bright text and a brass-hi icon. The footer always shows sync state (cloud icon, green or ember) and a lock button with its shortcut (Ctrl L / ⌘L); the lock icon tilts on hover.

### Toast
A glass pill centred 22px above the bottom: blurred dark ground in both themes, bright text, a green check or ember alert, lift shadow; rises in and falls away.

### Emergency Kit (signature)
A paper sheet (paper-sheet, 6px, lift shadow) in every theme: a serif roman heading over a 2px paper-strong rule, the Secret Key in mono 17px on a paper-well, write-in lines, a QR block, a hairline foot. Printing hides everything but the sheet.

### Unlock (signature)
The shield-and-keyhole mark at 72px over the slowly turning guilloche rosette (brass at 8% opacity, masked to a circle), the serif headline with its one brass italic, a one-line reason, one field. The key in the mark turns 90° while unlocking.

### Browser extension
- **Popup (320px):** the vault list in miniature. A 48px bar with the mark, name and a lock-state pill (green when unlocked, brass when locked); the current site; rows of 32px serif-monogram avatars with title and username; Fill as a small brass button; the TOTP code as a mono brass-ink button on a brass wash. Rows rise in with a 40ms stagger.
- **In-page menu:** a 10px card on pane with a line-strong border; 34px head (mark, name, site right-aligned); 46px rows at 8px radius whose hover and keyboard focus take the brass wash, focus adding an inset 1.5px brass ring. Rows slide in 3px with a 30ms stagger.
- **Save prompt:** the same card at 340 x 138, question in 13.5px 600, detail line, secondary and brass primary buttons right-aligned.
- **Options page:** a 640px column on the pane with a radial raise: 48px mark, serif 30px h1, and the Settings grammar (group titles, 12px groups of rows, status dots).
- **Fonts:** the three families ship inside the extension package as Latin woff2 subsets (`src/fonts/fonts.css`), loaded from the extension's own origin.

### Named Rules
**The Row Is the Field Rule.** Editing never adds a box around an input. The grouped row is the field; focus marks the row with the selection wash and a brass ring.

**The Still Card Rule.** The in-page menu and save-prompt `.card` never animates opacity or transform. The IntersectionObserver v2 click guard watches it, and Chromium counts a card that is still fading or scaling in as not visible. Only its children animate.

**The Own-Origin Fonts Rule.** The extension loads fonts only from its own package (CSP `font-src 'self'`). No font CDN, no system display face standing in.

**The No Inline Style Rule.** The desktop CSP is `style-src 'self'` (the extension pages' too): no inline `<style>` elements or style attributes in markup. Dynamic values go through CSSOM style properties from React, as the generator's slider fill does.

**The Stillness Rule.** Under `prefers-reduced-motion: reduce` every transition and animation is off, on every desktop and extension surface, including the rosette and the TOTP ring.

## Do's and Don'ts

### Do:
- **Do** build details, forms and settings from inset groups (12px) of hairline-separated rows.
- **Do** set titles in Source Serif 4 roman (28px item and editor, 22px pane, 30px unlock).
- **Do** keep brass to selection, the one committing action, focus and the live TOTP code.
- **Do** keep the sidebar forest green and the Emergency Kit sheet paper in every theme.
- **Do** move on `cubic-bezier(0.32, 0.72, 0, 1)` at 160, 280 or 420ms, and turn every animation off under reduced motion.
- **Do** keep in-page frame CSS in lockstep with `content/frames.ts` (34px header, 46px rows, 8px padding, 340 x 138 save prompt).
- **Do** animate only the children of the in-page `.card`.
- **Do** load extension fonts from the package, and pass dynamic desktop styles through CSSOM, never inline style tags.
- **Do** give the sidebar top row a 38px height and 76px left inset when `data-platform="mac"`, and verify it on a real Mac before relying on it.

### Don't:
- **Don't** wrap each field in its own bordered card, or give editor inputs their own border or focus ring inside a row.
- **Don't** use the brass italic anywhere but the unlock headline.
- **Don't** set labels, headings or counts in mono.
- **Don't** fill large surfaces with brass or add a second accent.
- **Don't** animate opacity or transform on the in-page `.card`.
- **Don't** change frame heights or the save prompt size in CSS alone.
- **Don't** fetch fonts or styles from the network in the extension.
- **Don't** give flat groups, rows or in-page cards a shadow; shadows are for floating things.
- **Don't** put small all-caps tracked labels above headings.
