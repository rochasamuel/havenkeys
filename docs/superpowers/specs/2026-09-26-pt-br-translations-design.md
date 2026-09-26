# Portuguese (Brazil) translations — extension and desktop app

Date: 2026-09-26. Status: approved in conversation (user asked to go straight
to implementation).

## Goal

The browser extension and the desktop app speak English and Brazilian
Portuguese, and no screen clips, cuts or overflows text in either language.

## Language choice

* **Extension:** follows the browser UI language
  (`chrome.i18n.getUILanguage()`). Any `pt*` locale → `pt-BR`; everything
  else → `en`. No switcher.
* **Desktop app:** follows the OS language as the webview reports it
  (`navigator.languages` / `navigator.language`) by default. Settings gets a
  *Language* row: Automatic / English / Português (Brasil). The choice is a
  UI preference kept in the webview's `localStorage` (key `hk-locale`),
  because the unlock screen needs it before any vault is open and it is not
  sensitive. Storage access is guarded; failure means Automatic.

## Message files

Same pattern as `apps/web/src/i18n`: per app, `src/i18n/en.ts` holds every
string the UI shows; `src/i18n/pt-BR.ts` is typed `Messages` (the type of
`en`), so a missing or extra key fails the typecheck. Parameterised strings
are functions (`(n: number) => string`). A `resolveLocale(tags, pref)` pure
function picks the locale and is unit-tested.

* **Extension:** `apps/extension/src/i18n/` exports `t` (resolved once per
  JS context) and `locale`. Popup, inline menu, save prompt, passkey prompt,
  options page, the content script's visible strings (field icon label,
  in-page titles) and the ~30 user-visible background messages read from
  it. Static text in HTML moves to the scripts (or `data-i18n` keys filled at
  load); each page sets `document.documentElement.lang`.
* **Manifest:** `_locales/en` and `_locales/pt_BR` contain only `name` and
  `description`; the manifest uses `__MSG_extName__` / `__MSG_extDescription__`
  and `default_locale: "en"`. Store listing text follows.
* **Desktop:** `apps/desktop/src/i18n/` with a React context
  (`I18nProvider`, `useI18n()`), every view and component converted.
* **Rust-originated text:** errors reach the UI as `{ code, message }`. The UI
  shows a translated message per known `code`, and falls back to Rust's
  English `message` for an unknown code. Rust stays English.
* **Tray menu:** a new Tauri command `set_ui_language(lang)` accepts only
  `"en" | "pt-BR"` (anything else is rejected) and relabels the three tray
  items from a fixed table in Rust. Added to the command allowlist/capability.
  No new Rust dependency.

Never translated: product names shown in the app (HavenKeys, Secret Key,
Emergency Kit, havenkeys-server), URLs, keyboard keys. Terminology: senha,
cofre (vault), login, código de verificação (one-time code), chave de acesso
(passkey), Chave Secreta is NOT used (Secret Key stays English).

## Layout hardening

Longer Portuguese strings must never clip. Buttons and labels wrap or grow
(`min-width` rather than fixed widths; no `white-space: nowrap` together with
`overflow: hidden` on labels); the inline menu's height calculation accounts
for wrapped rows; truncation with ellipsis is allowed only for user data
(titles, usernames, URLs), never for UI copy.

## Visual verification

A Playwright harness (dev dependency only; not shipped) renders:

* every extension page (popup states, inline menu variants, save prompt,
  passkey prompt, options) with `chrome.*` stubbed, at their real sizes;
* every desktop screen (welcome, unlock, vault list/detail/editor, generator,
  settings sections) with Tauri `invoke` stubbed, at the minimum window size
  and a typical size;

in both locales and both themes, saving screenshots, and **fails** when a
visible element that hides overflow has clipped text
(`scrollWidth > clientWidth + 1` or `scrollHeight > clientHeight + 1`,
excluding elements marked as user-data truncation) or any element spills out
of the viewport horizontally. Screenshots are reviewed by eye as well.

## Security

No change to trust boundaries. Translation strings are static and never
built from page or vault data with HTML: all insertion stays `textContent`.
`set_ui_language` takes a closed enum. Nothing new is persisted except the
non-sensitive locale preference.

## Tests

Typecheck enforces key parity. Unit tests: locale resolution (browser/OS tags
and preference), desktop error-code translation fallback, `set_ui_language`
rejecting unknown values. Existing tests keep running in English. The
Playwright overflow check runs as its own script (`pnpm ui:check`).
