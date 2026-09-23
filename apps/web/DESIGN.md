---
name: HavenKeys (havenkeys.net)
description: The marketing and download site for a password manager you run yourself; a forest-green, brass-fitted world with one paper chapter.
colors:
  ink: "#080d0b"
  ground: "#0b110f"
  bg: "#0f1614"
  bg-list: "#121b18"
  raised: "#172320"
  hover: "#1b2925"
  selected: "#1f2f2a"
  line: "#1f2c28"
  line-strong: "#2c3d37"
  text: "#e2eae6"
  text-strong: "#f3f7f5"
  muted: "#86968f"
  muted-hi: "#a3b2ab"
  brass: "#c9a45c"
  brass-hi: "#e3c483"
  brass-hover: "#d8b56f"
  brass-soft: "rgba(201, 164, 92, 0.14)"
  brass-line: "rgba(201, 164, 92, 0.38)"
  on-brass: "#121a17"
  cipher-green: "#8fc4ad"
  ok: "#5fa785"
  danger: "#e0775f"
  paper: "#e6ece8"
  paper-sheet: "#fbfcfb"
  paper-text: "#3a4842"
  paper-strong: "#121a17"
  paper-muted: "#5a6a63"
  paper-line: "#c7d2cc"
  paper-brass: "#7f6430"
typography:
  display:
    fontFamily: "Source Serif 4 Variable, Source Serif 4, Georgia, serif"
    fontSize: "clamp(2.75rem, 6.6vw, 5.6rem)"
    fontWeight: 440
    lineHeight: 1.02
    letterSpacing: "-0.022em"
    fontVariation: "opsz auto"
  headline:
    fontFamily: "Source Serif 4 Variable, Source Serif 4, Georgia, serif"
    fontSize: "clamp(2.1rem, 4.3vw, 3.6rem)"
    fontWeight: 440
    lineHeight: 1.06
    letterSpacing: "-0.022em"
  title:
    fontFamily: "Source Serif 4 Variable, Source Serif 4, Georgia, serif"
    fontSize: "clamp(1.9rem, 3vw, 2.65rem)"
    fontWeight: 500
    lineHeight: 1.15
    letterSpacing: "-0.01em"
  title-sm:
    fontFamily: "Source Serif 4 Variable, Source Serif 4, Georgia, serif"
    fontSize: "1.28rem"
    fontWeight: 520
    lineHeight: 1.2
  emphasis:
    fontFamily: "Source Serif 4 Variable, Source Serif 4, Georgia, serif"
    fontWeight: 380
    fontFeature: "italic"
  lede:
    fontFamily: "Hanken Grotesk Variable, Hanken Grotesk, system-ui, sans-serif"
    fontSize: "clamp(17px, 1.6vw, 19.5px)"
    fontWeight: 500
    lineHeight: 1.6
  body:
    fontFamily: "Hanken Grotesk Variable, Hanken Grotesk, system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 500
    lineHeight: 1.6
  label:
    fontFamily: "Hanken Grotesk Variable, Hanken Grotesk, system-ui, sans-serif"
    fontSize: "12.5px"
    fontWeight: 560
    lineHeight: 1.45
  mono:
    fontFamily: "JetBrains Mono, ui-monospace, monospace"
    fontSize: "14px"
    fontWeight: 500
    letterSpacing: "0.04em"
rounded:
  r-xs: "2px"
  r-sm: "5px"
  r-md: "8px"
  r-lg: "10px"
  frame: "12px"
  panel: "14px"
  column: "16px"
  stage: "18px"
  pill: "999px"
spacing:
  gutter: "clamp(16px, 4vw, 40px)"
  section: "clamp(96px, 12vw, 168px)"
  max: "1200px"
  head-gap: "clamp(48px, 6vw, 80px)"
  split-gap: "clamp(40px, 6vw, 88px)"
components:
  button-primary:
    backgroundColor: "{colors.brass}"
    textColor: "{colors.on-brass}"
    rounded: "{rounded.r-md}"
    padding: "0 18px"
    height: "42px"
  button-primary-hover:
    backgroundColor: "{colors.brass-hover}"
    textColor: "{colors.on-brass}"
  button-primary-lg:
    backgroundColor: "{colors.brass}"
    textColor: "{colors.on-brass}"
    rounded: "{rounded.r-md}"
    padding: "0 24px"
    height: "52px"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.r-md}"
    padding: "0 18px"
    height: "42px"
  button-ghost-hover:
    backgroundColor: "{colors.hover}"
  tag:
    backgroundColor: "transparent"
    textColor: "{colors.muted-hi}"
    rounded: "{rounded.pill}"
    padding: "2px 9px"
  tag-brass:
    textColor: "{colors.brass-hi}"
    rounded: "{rounded.pill}"
    padding: "2px 9px"
  panel:
    backgroundColor: "{colors.ground}"
    textColor: "{colors.muted-hi}"
    rounded: "{rounded.panel}"
    padding: "22px 26px"
  disclaimer:
    backgroundColor: "{colors.ground}"
    textColor: "{colors.muted-hi}"
    rounded: "{rounded.frame}"
    padding: "16px 20px"
  nav:
    backgroundColor: "rgba(11, 17, 15, 0.82)"
    textColor: "{colors.text}"
    height: "64px"
  showcase-tab-selected:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text-strong}"
    rounded: "{rounded.pill}"
    padding: "9px 18px"
  kit-sheet:
    backgroundColor: "{colors.paper-sheet}"
    textColor: "{colors.paper-text}"
    rounded: "4px"
    padding: "36px"
---

# Design System: HavenKeys (havenkeys.net)

## Overview

**Creative North Star: "The Brass-Fitted Strongroom"**

The site is a dark forest-green room with one metal fitting. Grounds step from near-black green (ink, ground, bg) up through raised panels, all divided by hairline green rules; brass is the only accent and it marks the one thing that commits (Download), the active state, and the human voice in the three headlines that carry it. An engraved guilloche rosette, drawn in brass at about a tenth of its strength, sits behind the hero and the closing call, the banknote pattern that says "hard to forge" without a word. One chapter, the Emergency Kit, changes the whole field to pale paper: the only light surface on the site, and the only place a sheet of paper is shown as an object.

Type is a conversation between a serif and a grotesque. Source Serif 4, with optical sizing, sets every heading in roman at a light 440 weight; its italic, in brass, is the human voice and is spent in three places only. Hanken Grotesk carries all reading text at a relaxed 1.6 line height. JetBrains Mono appears only where the visitor is looking at a literal machine string. The product is shown, not described: real extension renders, framed windows, and a sticky journey stage that moves one fictional login through four states.

This site builds on the product's shared token layer (`@havenkeys/ui/tokens.css`, dark theme): surfaces, lines, text, brass, ok/danger, radii, the mono and sans stacks and the mechanical easing are inherited from the desktop app and must not be redefined here. The site adds its own layer on `:root` in `src/styles/global.css`: the serif voice, a deeper ink and ground, a brighter muted (`muted-hi`) for long reading on dark, the paper palette, a brass hairline, the `lift` shadow, and the page measures (max, gutter, section).

**Key Characteristics:**
- Dark green grounds with brass as the single accent; one paper field per page at most.
- Serif roman headings; brass italic emphasis reserved for three moments.
- Hairline 1px green rules instead of cards-on-cards; dashed rules mark what is outside the trust boundary or not defended.
- Framed product windows with a deep, soft lift shadow; real screenshots overlap their edges.
- One orchestrated motion (the journey stage); everything else is static or a short state transition.

## Colors

A near-monochrome forest-green ramp with a single brass fitting, plus one pale paper field.

### Primary
- **Fitting Brass** (brass): the primary button fill, focus rings, the guilloche rosette, chain and flow lines, and the active journey rail dot. Never a large fill beyond a button.
- **Lit Brass** (brass-hi): brass that carries text or a hairline dot: italic heading emphasis, links, active nav, brass tags, placeholder labels.
- **Brass Wash** (brass-soft) and **Brass Hairline** (brass-line): the tint and border of the one highlighted box in a group (the vault-key output, the Rust core zone, the visitor's own platform card, the sealed item).

### Secondary
- **Cipher Green** (cipher-green, the product's `symbol` token): ciphertext. Plaintext turns this color when it is sealed; stored blobs are listed in it.

### Tertiary
- **Leaf OK** (ok) and **Ember** (danger): check and cross icons in the defends / doesn't-defend ledger, allowed and denied origins. Icon color only; never a fill.

### Neutral
- **Ink** (ink): the footer, the deepest ground.
- **Forest Ground** (ground): hero top, stage, panels, tables, disclaimers, the closer.
- **Window Green** (bg): the page body between sections.
- **List Green / Raised Green** (bg-list, raised): cards inside a scene, inline code, the selected showcase tab.
- **Hairline / Strong Hairline** (line, line-strong): every divider and every interactive border.
- **Bone / Bright Bone** (text, text-strong): body copy and headings.
- **Lichen / Pale Lichen** (muted, muted-hi): captions and meta; `muted-hi` is the reading color for ledes and paragraphs on dark.
- **Paper set** (paper, paper-sheet, paper-text, paper-strong, paper-muted, paper-line, paper-brass): the Emergency Kit field only. On paper, brass darkens to `paper-brass` or it disappears.

### Named Rules
**The One Fitting Rule.** Brass is the only accent. A screen has one brass-filled control at most (Download); everything else brass is a hairline, a dot, a word or a wash.

**The Inherited Palette Rule.** Surface, line, text, brass, ok and danger values come from `@havenkeys/ui` tokens. The site adds only the `:root` site layer; a new hex in component CSS is drift, not a new token.

**The Foreign Page Rule.** The simulated website inside the browser showcase and the fill scene (white card, blue focus #3b6cf6, system font) is a page HavenKeys does not own, so it deliberately uses none of this palette. That styling stays inside the browser frame.

## Typography

**Display Font:** Source Serif 4 Variable (with Source Serif 4, Georgia, serif), optical sizing on
**Body Font:** Hanken Grotesk Variable (with system-ui, sans-serif)
**Label/Mono Font:** JetBrains Mono (with ui-monospace, monospace)

**Character:** A bookish, slightly light serif speaking in plain sentences over a warm, precise grotesque; mono is evidence, never decoration.

### Hierarchy
- **Display** (440, clamp(2.75rem, 6.6vw, 5.6rem), 1.02): the page's single h1, balanced, centered.
- **Headline** (440, clamp(2.1rem, 4.3vw, 3.6rem), 1.06): section h2s; the closer runs larger at clamp(2.6rem, 5.8vw, 4.8rem).
- **Title** (500, clamp(1.9rem, 3vw, 2.65rem), 1.15): journey chapter heads; smaller h3s (1.3 to 2.2rem) for ledger columns, zones, setup steps and showcase copy.
- **Small serif title** (520, 1.2 to 1.35rem): definition terms, chain nodes, reading-list titles.
- **Lede** (clamp(17px, 1.6vw, 19.5px), max 62ch, muted-hi): the sentence under a hero or section head (section heads use 18px, 60ch).
- **Body** (500, 16px, 1.6): all prose; chapter paragraphs at 17.5px, 46ch.
- **Label** (560, 12.5 to 13.5px, sentence case): field labels, rail steps, zone terms. No all-caps tracked labels.

### Named Rules
**The Three Italics Rule.** The italic brass emphasis (Source Serif italic, 380, brass-hi) appears only in the hero headline, the four journey chapter heads, and the final call-to-action. Every other heading on every page, including the paper chapter, is roman with no emphasis.

**The Literal String Rule.** Mono is only for strings a machine produced or a person must type or match exactly: keys, ciphertext, commands, domains, and the identifiers around them (KDF operation names, permission strings, file paths, release tags). Never for headings, numbers-as-decoration or labels.

## Layout

A centered 1200px column (`max`) with fluid gutters (clamp 16 to 40px) and a large, fluid section rhythm (clamp 96 to 168px) between sections. Section heads are centered, max 780px, with a fluid gap to their content; a left-aligned head variant pairs with content in a two-column split (5fr / 6fr or 5fr / 7fr, gap clamp 40 to 96px).

The home page's signature layout is the journey: four chapters, each at least 78vh, beside a sticky 600px stage that follows the reader. Below 900px the stage is removed and each chapter carries its own inline scene, separated by hairlines. Below 640px the browser showcase frame is swapped for the bare screenshot, hero buttons go full width, and the in-page menu overlay drops out of the hero.

Inner pages (Security, Download) open with a centered page hero (max 900px) and then use the same sections; legal pages (Privacy, Terms) read in a 720px column at 17px / 1.7.

## Elevation & Depth

Depth is mostly tonal: grounds step from ink to raised, and hairlines separate. Shadows are reserved for objects that sit above the page as physical things: product windows, overlaid screenshots, the stage and the printed kit. They are always soft and offset downward, never hard-edged.

### Shadow Vocabulary
- **Lift** (`box-shadow: 0 40px 80px -24px rgba(0,0,0,0.65), 0 12px 24px -12px rgba(0,0,0,0.5)`): framed windows, the browser frame, the hero popup screenshot.
- **Stage** (`box-shadow: 0 30px 60px -30px rgba(0,0,0,0.6)`): the journey stage.
- **Screenshot drop** (`filter: drop-shadow(0 24px 34px rgba(0,0,0,0.5))`): transparent extension renders, so the shadow follows their shape.
- **Paper sheet** (`0 1px 0 #d6ded9, 0 36px 60px -24px rgba(22,35,31,0.38), 0 8px 18px -8px rgba(22,35,31,0.2)`): the Emergency Kit, on the paper field only.
- **Brass hover glow** (`0 10px 22px -10px rgba(201,164,92,0.6)`): primary button hover only.

### Named Rules
**The Objects-Only Shadow Rule.** Only things that represent a real object (an app window, a screenshot, a sheet of paper, the stage) cast a shadow. Text panels, tables, ledger columns and disclaimers stay flat on a tonal ground with a hairline.

## Shapes

Gently rounded, from the product's radius scale: controls at 8px, scene cards at 10px, frames and screenshots at 12px, panels, tables and windows at 14px, ledger columns at 16px, the stage at 18px. Pills (999px) are for tags and the showcase tab group only. The kit sheet is nearly square-cornered (4px) and tilted -1.6deg, the one rotated object on the site (untilted on mobile).

Borders are 1px. Solid hairlines mean inside or defended; dashed hairlines mean outside, not defended, or placeholder (the "Not defended" column, zone separators, the placeholder tag, stored blob rows). Flow and chain lines are 1px verticals with small hollow or filled dots at the nodes, graded from brass to line-strong.

## Components

### Buttons
- **Shape:** gently rounded (8px), 42px tall; small 36px, large 52px.
- **Primary:** brass fill, dark on-brass text, 600 weight, with a leading icon (Download). One per view.
- **Hover / Focus:** primary lifts 1px, fills brass-hover and gains the brass glow; ghost gains a brass hairline and hover ground. Focus is a 2px brass-hi outline offset 3px. Transitions 160ms on the product's mechanical easing.
- **Ghost:** transparent, line-strong border, text-strong label, trailing arrow icon.

### Chips (tags)
- **Style:** pill, 1px line-strong border, 12px 560 label in muted-hi.
- **State:** the brass variant (brass-line border, brass-hi text) marks optional or "yours"; no filled chips.

### Cards / Containers
- **Corner Style:** 14px panels, 16px ledger columns, 10px scene cards.
- **Background:** ground on bg; raised inside a stage.
- **Shadow Strategy:** flat; see Elevation.
- **Border:** 1px line; the single highlighted item in a group takes brass-line and brass-soft.
- **Internal Padding:** 16 to 36px, fluid on large columns.

### Navigation
Sticky 64px bar over a translucent ground with blur and a bottom hairline. Brand mark and wordmark at left (660 weight); 15px links in text color turning brass-hi on hover and when active; a small primary Download button at the end. Below 640px secondary links and the wordmark hide.

### Desktop Placeholder (signature)
Desktop app screenshots do not exist yet, so every desktop slot uses `DesktopPlaceholder`: a framed window with a title bar and a skeleton of the real three-pane vault layout, overlaid with a dashed brass tag that names what screenshot belongs there. Never a stock image, a mock UI with invented data, or an unlabeled grey box.

### Extension Screenshots (signature)
The PNGs in `src/assets/shots` are real renders of the extension UI (popup, in-page menus, save prompt), placed with a lift or drop shadow, overlapping a window edge in the hero and floating inside the browser showcase frame. They are shown at their real size or smaller, never redrawn.

### Journey Stage (signature)
A sticky, 18px-rounded ground panel with a four-step rail (Typed, Sealed, Stored, Filled) at the top; the active step is brass-hi, completed steps fill their dot. Scenes cross-fade with a small rise and blur (520 to 620ms, mechanical easing); in the sealed step plaintext visibly scrambles into cipher-green ciphertext. This is the site's one orchestrated motion; reduced motion shows every scene settled.

### Emergency Kit (signature)
On the paper field, a white sheet with a heavy 2px rule under the header, an italic serif document title in paper-brass, the Secret Key in mono on a pale well, a QR block and blank write-in lines.

## Do's and Don'ts

### Do:
- **Do** take surface, text, line, brass and status colors from `@havenkeys/ui` tokens, and add site-only values to the `:root` site layer, never inline in a component rule.
- **Do** keep brass italic emphasis to the hero headline, the journey chapter heads and the final call-to-action.
- **Do** use mono only for keys, ciphertext, commands, domains and the literal identifiers around them.
- **Do** use the fictional `fernway.example` (and `example.com` addresses) for all demo data, and label it where it appears ("demo site", "demo data").
- **Do** fill any missing desktop screenshot with `DesktopPlaceholder` and a label saying what goes there.
- **Do** use only real extension renders from `src/assets/shots` for extension imagery.
- **Do** ship styles only through the bundled stylesheet and images only as emitted files: the CSP is `style-src 'self'; img-src 'self'`, so no inline `<style>` elements and no `data:` images, and Vite's `assetsInlineLimit` stays at 0. Dynamic values go through React style props (CSSOM), as the stage delays and showcase scale do.
- **Do** carry the verbatim audit disclaimer in a flat ground panel wherever security claims are summarized.

### Don't:
- **Don't** add a second accent color or fill large areas with brass.
- **Don't** set any other heading in italic or brass, including the paper chapter heading.
- **Don't** use real brands or real people as demo accounts, or leave demo data unlabeled.
- **Don't** invent desktop UI in place of a screenshot, or redraw the extension UI in HTML.
- **Don't** put small all-caps tracked labels above headings; headings stand on their own.
- **Don't** give flat text panels, tables or ledger columns a shadow; shadows are for objects.
- **Don't** add a second scroll-driven animation; the journey stage is the one orchestrated motion.
- **Don't** use a light field anywhere except a single paper chapter.

## Copy and languages

Copy is not written in components. Every string lives in `src/i18n/en.tsx` and `src/i18n/pt-BR.tsx` (English at `/`, Brazilian Portuguese under `/pt-br`). The italic-emphasis rule applies in both languages: the `<em>` sits inside the dictionary's hero, journey chapter and closing headings, and nowhere else. Portuguese runs about 15–25% longer, so check new copy at 360px in `pt-BR` before shipping: tab labels, the seal card's labels and the nav are the tightest places.

