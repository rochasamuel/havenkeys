# Portuguese (Brazil) Translations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** The extension and the desktop app are fully available in English and Brazilian Portuguese, with no clipped or overflowing text in either.

**Spec:** `docs/superpowers/specs/2026-09-26-pt-br-translations-design.md`

**Tech Stack:** TypeScript (MV3 extension, vitest + jsdom), React 19 (Tauri desktop), Rust (Tauri tray), Playwright (dev only, visual check).

## Global Constraints

- Locales: `en`, `pt-BR`. Resolution: first tag starting with `pt` (case-insensitive) → `pt-BR`, else `en`. Desktop preference `"auto" | "en" | "pt-BR"` in `localStorage["hk-locale"]` (guarded; missing/invalid/throwing → `"auto"`).
- Message files: `src/i18n/en.ts` exports `en` and `type Messages = typeof en` (widened so string literals are `string`); `src/i18n/pt-BR.ts` exports `ptBR: Messages`. Parameterised strings are functions. Pattern reference: `apps/web/src/i18n/`.
- Never translate: HavenKeys, Secret Key, Emergency Kit, havenkeys-server, URLs, key names. Terms: senha, cofre, login, código de verificação, chave de acesso (passkey).
- Portuguese must be natural Brazilian Portuguese (você, not tu), concise UI copy, sentence case like the English.
- All DOM insertion stays `textContent` / React text. No `innerHTML`, no `dangerouslySetInnerHTML`. Never log.
- Existing tests keep passing in English (tests may force `en`).
- Commits: no `Co-Authored-By` trailer.
- UI copy is never truncated with ellipsis; only user data (titles, usernames, URLs) may be.

---

### Task 1: Extension i18n core + manifest locales

**Files:** create `apps/extension/src/i18n/{en.ts,pt-BR.ts,index.ts,locale.ts,locale.test.ts}`, `apps/extension/manifest/_locales/{en,pt_BR}/messages.json` (or wherever build.mjs copies from — wire it into `build.mjs` so both dist targets get `_locales/`); modify `manifest/base.json` (`default_locale: "en"`, name/description → `__MSG_extName__` / `__MSG_extDescription__`).

- `locale.ts`: `export type Locale = "en" | "pt-BR"; export function resolveLocale(tags: readonly string[]): Locale`.
- `index.ts`: `export const locale: Locale` computed once from `chrome.i18n?.getUILanguage?.()` falling back to `navigator.languages`; `export const t: Messages`; `export function applyDocumentLang(): void` sets `document.documentElement.lang`. Tests can import `en` directly.
- Start `en.ts` / `pt-BR.ts` with the manifest-independent sections the later tasks fill (it is fine to create them nearly empty and let Tasks 2–4 add their sections).
- Tests: `resolveLocale(["pt-BR"])`, `["pt"]`, `["PT-pt"]` → pt-BR; `["en-US"]`, `["es"]`, `[]` → en.
- Verify: extension test, typecheck, build; `dist/chrome/_locales/pt_BR/messages.json` exists and the built manifest keeps `__MSG_` placeholders valid.

### Task 2: Extension popup + options page

**Files:** `apps/extension/src/popup/*`, `apps/extension/src/options/*`, `src/i18n/{en,pt-BR}.ts`.

Move every visible string (including `title`, `aria-label`, `placeholder`, button text, status/error text, empty states) in the popup and options page into `t.popup.*` / `t.options.*`, with Portuguese. Static HTML text is filled from `t` at load (or created by script); call `applyDocumentLang()`. Update tests that assert English text only if needed (keep them passing under `en`).

### Task 3: Inline menu, save prompt, passkey prompt, content-script strings, background messages

**Files:** `apps/extension/src/menu/*`, `apps/extension/src/content/{index,icon,frames}.ts` (visible strings only), `apps/extension/src/background/*.ts` (user-visible `message:` strings), `src/i18n/*`.

Same conversion for `t.menu.*`, `t.save.*`, `t.passkey.*`, `t.content.*`, `t.errors.*`. Background messages that reach the UI use `t.errors.*` (the background resolves its own locale). Keep message/protocol shapes unchanged. Review `menuBox`/row-height math in the content script so wrapped rows in Portuguese are not clipped (compute from content or allow the frame to grow).

### Task 4: Desktop i18n core, Language setting, tray command

**Files:** create `apps/desktop/src/i18n/{en.ts,pt-BR.ts,locale.ts,locale.test.ts,context.tsx,preference.ts}`; modify `apps/desktop/src/main.tsx`/`App.tsx` (wrap in provider), `views/SettingsView.tsx` (Language row), `src-tauri/src/tray.rs` + command registration + capability/allowlist for `set_ui_language`, `src/lib/api.ts` (wrapper), Rust unit test.

- `preference.ts`: `readPreference(): "auto"|"en"|"pt-BR"`, `writePreference(p)`, guarded.
- `context.tsx`: `I18nProvider` resolves `pref === "auto" ? resolveLocale(navigator.languages) : pref`, sets `document.documentElement.lang`, calls `api.setUiLanguage(locale)` (ignore failures), exposes `{ locale, t, preference, setPreference }`.
- Settings row "Language" / "Idioma" with a select: Automatic (Automático) / English / Português (Brasil). Changes apply immediately.
- Rust `set_ui_language(lang: String)`: accept only `"en"` / `"pt-BR"` (else error with a fixed code), relabel tray items `open` / `lock` / `quit` from a fixed table (pt-BR: "Abrir HavenKeys", "Bloquear", "Sair do HavenKeys"). Keep menu item handles in managed state as needed.
- Tests: resolution + preference (TS), `set_ui_language` rejects unknown values (Rust).

### Task 5: Desktop screens — unlock, welcome, vault list, item detail, item editor, generator

**Files:** `apps/desktop/src/views/{WelcomeScreen,UnlockScreen,VaultScreen,ItemList,ItemDetail,ItemEditor,GeneratorView}.tsx`, components (`CopyButton`, `EmergencyKit`, `Toast`, `Switch` labels, `Seal`, …), `lib/format.ts` (relative dates/labels if any), `src/i18n/*`.

Convert every visible string, including aria-labels/titles/placeholders and toast text. Dates/numbers use `Intl` with the active locale where they are formatted.

### Task 6: Desktop screens — settings, account, import; error-code translation

**Files:** `views/{SettingsView,AccountSection,ImportSection}.tsx`, `lib/api.ts` or a new `i18n/errors.ts`, `src/i18n/*`, test.

Convert remaining views. Add `errorMessage(e: unknown, t: Messages): string`: `ApiError` with a known `code` → translated text; unknown code → Rust's English `message`; non-ApiError → generic translated message. List the codes by reading `src-tauri/src/*.rs` and the core's error codes. Replace UI call sites that display `e.message`.

### Task 7: Layout hardening + Playwright visual check

**Files:** CSS of extension (`popup.css`, `options.css`, `menu/inline.css`, `theme.css`) and desktop (`styles.css`, packages/ui if used); create `tools/ui-check/` (or `apps/*/ui-check/`) with a Playwright script + fixtures stubbing `chrome.*` and Tauri `window.__TAURI_INTERNALS__.invoke`; root script `pnpm ui:check`; add `@playwright/test` or `playwright` as a root devDependency (use the already cached Chromium if the version matches, otherwise install).

- Render every extension page state and every desktop screen in `en` and `pt-BR`, light and dark, at real sizes (popup width, inline menu frame sizes, desktop min window size from `tauri.conf.json` and a typical size). Save PNGs to a git-ignored `ui-check-output/`.
- Fail on clipped UI text (see spec) or horizontal overflow of the viewport. Elements that legitimately truncate user data carry `data-truncate` (add it where appropriate) and are excluded.
- Fix every failure by adjusting CSS (wrap/grow), not by shortening translations unless the Portuguese is needlessly long.
- Review the screenshots visually; fix anything that looks broken even if the check passes (misaligned buttons, awkward wraps).

### Task 8: Docs

**Files:** `docs/development.md` (how to add a string; `pnpm ui:check`), `docs/architecture.md` or README (languages supported), `docs/security-model.md` (the `hk-locale` localStorage key is a non-sensitive UI preference; `set_ui_language` command).

Full verification: `pnpm -r typecheck && pnpm -r test && cargo test -p havenkeys-desktop && cargo clippy --workspace --all-targets -- -D warnings && pnpm ui:check && pnpm --filter @havenkeys/extension build`.
