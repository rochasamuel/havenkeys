// HavenKeys design tokens.
//
// The UI runs in three places: the Tauri window (React), the extension's
// in-page frames (plain DOM), and eventually a phone. Only the first can
// import TypeScript, so the values are authored here once and the shipping
// artefact is plain CSS custom properties — see src/tokens.css, written by
// src/generate.ts. That is also why this package has no runtime dependency
// and no React: a token layer that needs a framework to resolve is not a
// token layer the extension can use.
//
// Every value below already appears in apps/desktop/src/styles.css (the font
// stack for pages we do not control comes from apps/extension/src/menu/
// inline.css). This file extracts the app's existing visual language; it does
// not redesign it. Where the app has no value for something, the token is
// absent rather than guessed.

/** A CSS value, written exactly as it appears in the stylesheet. */
export type Css = string;

// ------------------------------------------------------------------ colour

/**
 * The dark theme, which is also the default. Keys are CSS custom property
 * names without the leading `--`; the generator prefixes them.
 *
 * `shadow` sits with the colours rather than in `elevation` because it is the
 * one elevation value that changes with the theme, and what changes about it
 * is the colour.
 */
const dark = {
  /** The window, and the detail pane behind everything. */
  bg: "#0f1614",
  /** The sidebar. Dark in every theme — see the `side-*` tokens below. */
  "bg-side": "#0b110f",
  /** The middle column that lists items. */
  "bg-list": "#121b18",
  /** The surface a card, input, toast or menu sits on. */
  raised: "#172320",
  /** A row or button under the pointer. */
  hover: "#1b2925",
  /** The row the user is currently on. */
  selected: "#1f2f2a",
  /** A divider between panes and between rows. */
  line: "#1f2c28",
  /** The edge of something interactive: a button or input border. */
  "line-strong": "#2c3d37",
  /** Body text. */
  text: "#e2eae6",
  /** A title, or the value the user opened the item to read. */
  "text-strong": "#f3f7f5",
  /** A label, a count, anything secondary to the line it sits on. */
  muted: "#86968f",
  /** The fitting: focus rings, and the accent on whatever is active. */
  brass: "#c9a45c",
  /** Brass lifted for hover, or brass that has to carry text. */
  "brass-hi": "#e3c483",
  /** A brass wash behind an avatar or badge. */
  "brass-soft": "rgba(201, 164, 92, 0.14)",
  /**
   * Brass that has to carry text. On paper the fitting must darken or it
   * disappears, so unlike `brass` this one follows the theme. Same pair the
   * generated-password digits use.
   */
  "brass-ink": "#e3c483",
  /** The one button that commits the screen's action. */
  "primary-bg": "#c9a45c",
  /** Text on `primary-bg`. */
  "primary-fg": "#121a17",
  /** `primary-bg` under the pointer. */
  "primary-hover": "#d8b56f",
  /** An error, or an action that destroys something. */
  danger: "#e0775f",
  /** A confirmation, or a strength bar that is long enough. */
  ok: "#5fa785",
  /** Digits inside a generated password, so they read apart from letters. */
  digit: "#e3c483",
  /** Symbols inside a generated password. */
  symbol: "#8fc4ad",
  /** The square standing in for an item that has no icon. */
  "avatar-bg": "#1d2b27",
  /** The initial drawn in that square. */
  "avatar-fg": "#e3c483",
  /** Text on the sidebar, which never follows the theme. */
  "side-text": "#c3cfc9",
  /** A secondary label on the sidebar. */
  "side-muted": "#7f8f88",
  /** A sidebar row under the pointer. */
  "side-hover": "#16211e",
  /** The sidebar row for the section the user is in. */
  "side-selected": "#1d2b27",
  /** A divider inside the sidebar. */
  "side-line": "#1c2825",
  /** Text on a brass button. Brass is brass in both themes, so this is too. */
  "on-brass": "#121a17",
  /** Text on a danger button. */
  "on-danger": "#ffffff",
  /** A surface floating over the window: a menu, a toast, the in-page card. */
  shadow: "0 12px 32px rgba(0, 0, 0, 0.45)",
} as const satisfies Readonly<Record<string, Css>>;

/** Every colour a theme must supply. All three themes define all of them. */
export type ColorToken = keyof typeof dark;

export type ThemeColors = Readonly<Record<ColorToken, Css>>;

/**
 * The light theme. `brass`, `brass-hi` and the `side-*` colours repeat their
 * dark values: today the desktop lets them fall through from `:root`, which
 * works but means no single block describes a theme. Repeating them is a
 * no-op in the browser and makes each block readable on its own.
 */
const light: ThemeColors = {
  bg: "#ffffff",
  "bg-side": "#16231f",
  "bg-list": "#f4f7f5",
  raised: "#ffffff",
  hover: "#e9efec",
  selected: "#ffffff",
  line: "#e2e8e5",
  "line-strong": "#cbd5d0",
  text: "#1e2b27",
  "text-strong": "#121a17",
  muted: "#66756f",
  brass: "#c9a45c",
  "brass-hi": "#e3c483",
  "brass-soft": "rgba(201, 164, 92, 0.16)",
  "brass-ink": "#8f7236",
  "primary-bg": "#16231f",
  "primary-fg": "#f1f5f3",
  "primary-hover": "#2b3f39",
  danger: "#b4412f",
  ok: "#3d6b58",
  digit: "#8f7236",
  symbol: "#3d6b58",
  "avatar-bg": "#16231f",
  "avatar-fg": "#e3c483",
  "side-text": "#c3cfc9",
  "side-muted": "#7f8f88",
  "side-hover": "#16211e",
  "side-selected": "#1d2b27",
  "side-line": "#1c2825",
  "on-brass": "#121a17",
  "on-danger": "#ffffff",
  shadow: "0 10px 28px rgba(22, 35, 31, 0.16)",
};

/** Value for the CSS `color-scheme` property: native controls, scrollbars. */
export type ColorScheme = "dark" | "light";

export interface Theme {
  readonly colorScheme: ColorScheme;
  readonly colors: ThemeColors;
}

/**
 * `systemLight` is what `[data-theme="system"]` resolves to when the OS asks
 * for light. It is a separate entry because it is emitted under its own
 * selector inside a media query, but it shares `light`'s values so the two
 * cannot drift apart by accident.
 */
export type ThemeName = "dark" | "light" | "systemLight";

export const themes = {
  dark: { colorScheme: "dark", colors: dark },
  light: { colorScheme: "light", colors: light },
  systemLight: { colorScheme: "light", colors: light },
} as const satisfies Readonly<Record<ThemeName, Theme>>;

/** The three themes in the order the stylesheet emits them. */
export const THEME_ORDER: readonly ThemeName[] = ["dark", "light", "systemLight"];

// ----------------------------------------------------------------- spacing

/**
 * The app's rhythm is 2px-based and dense. These are the gaps and paddings
 * that recur across panes; the odd numbers in styles.css (5, 7, 9, 11px) are
 * padding tuned to a specific control's height, not steps on a scale, so they
 * stay in the component CSS. 32px and 40px appear once each, also as layout
 * one-offs.
 */
export const spacing = {
  /** Icon to its label, or a stack of lines that belong to one thing. */
  "space-3xs": "2px",
  /** A label above the control it names. */
  "space-2xs": "4px",
  /** Items inline inside one control. */
  "space-xs": "6px",
  /** Between buttons in a row. */
  "space-sm": "8px",
  /** The default gap between two related controls. */
  "space-md": "10px",
  /** Between fields, and the horizontal padding of a pane. */
  "space-lg": "12px",
  /** Padding inside a card. */
  "space-xl": "14px",
  /** Padding at the edge of a pane. */
  "space-2xl": "16px",
  /** Between sections of a screen. */
  "space-3xl": "20px",
  /** Around a panel that owns the whole window. */
  "space-4xl": "24px",
  /** The breathing room above the unlock screen's seal. */
  "space-5xl": "28px",
} as const satisfies Readonly<Record<string, Css>>;

// ------------------------------------------------------------------- radii

/**
 * `r-sm` and `r-md` keep the names the desktop already uses. The extension's
 * in-page menu currently rounds at 6, 7 and 9px; those are within a pixel of
 * `r-sm`/`r-md` and should collapse onto them when it adopts these tokens.
 */
export const radii = {
  /** A hairline bar, like the password strength meter. */
  "r-xs": "2px",
  /** Buttons, inputs, list rows — anything the pointer lands on. */
  "r-sm": "5px",
  /** A card, panel or toast. */
  "r-md": "8px",
  /** A surface floating over a page we do not control. */
  "r-lg": "10px",
  /** A circle: only ever applied to a square. */
  "r-full": "50%",
} as const satisfies Readonly<Record<string, Css>>;

// -------------------------------------------------------------- typography

export const fontFamily = {
  /** Everything, unless it is a secret the user has to read character by character. */
  font: '"Hanken Grotesk Variable", "Hanken Grotesk", system-ui, sans-serif',
  /** Passwords, TOTP codes, recovery keys: anything read or typed exactly. */
  mono: '"JetBrains Mono", ui-monospace, monospace',
  /** Frames injected into pages we do not control, where our webfont is not loaded. */
  "font-system": 'system-ui, -apple-system, "Segoe UI", sans-serif',
} as const satisfies Readonly<Record<string, Css>>;

/**
 * Half-pixel sizes are deliberate: the app's base size is 13.5px and the
 * steps below it are the ones that stay legible at that density. 13px and
 * 16px appear in styles.css too, each in one component, and stay there.
 */
export const fontSize = {
  /** The smallest print we set: captions under a printed recovery kit. */
  "text-2xs": "11px",
  /** Field labels, counts, the note about how a URL matched. */
  "text-xs": "11.5px",
  /** The second line under a title, and the label above a control. */
  "text-sm": "12px",
  /** Dense controls: small buttons, banners, toasts. */
  "text-md": "12.5px",
  /** The app's reading size. */
  "text-base": "13.5px",
  /** The heading of a pane. */
  "text-lg": "14px",
  /** The product name, and the master-password box. */
  "text-xl": "15px",
  /** A section heading, and the generated password. */
  "text-2xl": "17px",
  /** The current TOTP code, and the heading of a sheet. */
  "text-3xl": "18px",
  /** The title of the item on screen. */
  "text-4xl": "19px",
  /** The one line the unlock screen leads with. */
  "text-5xl": "22px",
} as const satisfies Readonly<Record<string, Css>>;

/**
 * Hanken Grotesk is a variable font, so these are the exact axis positions
 * the app uses rather than a 100-step ladder. They are named for the job
 * because the differences between 550, 560 and 600 only make sense by job.
 */
export const fontWeight = {
  /** Body text, and monospace secrets, which look heavy at anything more. */
  "weight-body": "500",
  /** Button labels. */
  "weight-control": "550",
  /** A label, or the first line of a list row. */
  "weight-label": "560",
  /** A word that has to win inside a sentence. */
  "weight-emphasis": "600",
  /** The heading of a pane or settings block. */
  "weight-heading": "640",
  /** The title of the item on screen, and section headings. */
  "weight-title": "650",
  /** The product name in the sidebar. Used once, on purpose. */
  "weight-brand": "660",
} as const satisfies Readonly<Record<string, Css>>;

export const lineHeight = {
  /** Two lines that belong to one control and must not look like a list. */
  "leading-tight": "1.2",
  /** A short stack of labels. */
  "leading-snug": "1.25",
  /** The app's default. */
  "leading-base": "1.45",
  /** A textarea the user is typing into. */
  "leading-relaxed": "1.5",
  /** A secure note being read, where the line length is real. */
  "leading-prose": "1.55",
} as const satisfies Readonly<Record<string, Css>>;

export const letterSpacing = {
  /** The unlock headline, which is large enough to need pulling in. */
  "tracking-tight": "-0.01em",
  /** Initials in an avatar square. */
  "tracking-wide": "0.02em",
  /** Groups of a recovery key, so the eye can count them. */
  "tracking-wider": "0.04em",
  /** The TOTP digits, which are read aloud or retyped. */
  "tracking-code": "0.05em",
  /** The master-password box, where each keystroke should register. */
  "tracking-secret": "0.06em",
  /** A small all-caps label. */
  "tracking-caps": "0.08em",
  /** The dots standing in for a secret that is still hidden. */
  "tracking-masked": "0.18em",
} as const satisfies Readonly<Record<string, Css>>;

// --------------------------------------------------------------- elevation

/**
 * `shadow` is theme-scoped and lives with the colours. What is left is the
 * focus ring, which has to be visible on any surface: the inner band is the
 * page colour so the brass outer band reads as a ring rather than a glow.
 */
export const elevation = {
  /** The ring on whatever has keyboard focus. */
  focus: "0 0 0 2px var(--bg), 0 0 0 4px rgba(201, 164, 92, 0.6)",
} as const satisfies Readonly<Record<string, Css>>;

// ------------------------------------------------------------------ motion

export const duration = {
  /** A control moving under the pointer, like the lock latch. */
  "dur-snap": "240ms",
  /** The TOTP ring stepping one second. */
  "dur-tick": "250ms",
  /** The seal opening when the vault unlocks. */
  "dur-seal": "420ms",
  /** The idle loop on the seal while the vault is locked. */
  "dur-breath": "1.2s",
} as const satisfies Readonly<Record<string, Css>>;

export const easing = {
  /** Things that should move like hardware: fast off the mark, settling flat. */
  "ease-mechanical": "cubic-bezier(0.3, 0.7, 0.2, 1)",
  /** A loop that has no start or end. */
  "ease-breath": "ease-in-out",
  /** A countdown, where any curve would misreport the time left. */
  "ease-steady": "linear",
} as const satisfies Readonly<Record<string, Css>>;

// ------------------------------------------------------------------ layers

/**
 * Three layers, because three is all the app has needed. The page-top value
 * is the maximum a browser accepts: the extension's frames have to sit above
 * whatever the page stacked, and the page chose its numbers first.
 */
export const layers = {
  /** A menu opened from a button, above its own pane. */
  "z-popover": "10",
  /** Toasts, above everything else in the desktop window. */
  "z-toast": "50",
  /** The extension's in-page frames, above anything a page can stack. */
  "z-page-top": "2147483647",
} as const satisfies Readonly<Record<string, Css>>;
