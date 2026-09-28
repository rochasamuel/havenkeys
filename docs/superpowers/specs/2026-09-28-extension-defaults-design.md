# Extension on by default; suggestions are only the field menu — Design

Status: proposed, 2026-09-28.

> This software has not undergone an independent security audit.

## 1. Goal

Today one switch, the optional host permission `https://*/*` + `http://*/*`,
controls three things at once: the menu under login fields, the "Save /
Update this login?" prompt, and HavenKeys answering passkey requests. It is
off at install, so a fresh extension does none of them.

After this change:

* The extension has site access from install. Save/update prompts and
  passkeys work without the user turning anything on in the extension.
* "Suggestions in login fields" becomes a preference that only shows or
  hides the field icon and the menu under login fields. Turning it off
  leaves save/update prompts, passkeys, and filling from the toolbar popup
  working.
* The preference is on by default.

Success: on a fresh install (desktop integration switched on, vault
unlocked), signing in to a site with an unsaved password shows "Save this
login?". With suggestions turned off in the options page, the field menu
and icon disappear, and signing in with a changed password still shows
"Update the saved password?", and a passkey `create()` still opens the
HavenKeys save card.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| How site access is on by default | Move the patterns to required `host_permissions` | Keep optional and open a welcome page with an "Allow" button (not on without that click) |
| Where the suggestions preference lives | `chrome.storage.local`, key `inlineSuggestions`, one boolean; absent = on | Encode it in the host permission (current design, which couples it to saving and passkeys); desktop settings (the preference is per browser, and the desktop may be locked) |
| Who enforces it | Background refuses to open a menu when off; the content script also stops showing the icon and asking for a menu | Content script only (the background is the single place every menu goes through) |
| Desktop `browser_integration` default | **Unchanged: off.** | On by default: reverses security-review P6 and exposes vaults of users who never use the extension to same-user processes (P9) |
| What the preference does not touch | Save/update prompts, passkeys, toolbar popup fill, auto sign-in started from the popup | — |
| Generator menu on sign-up fields (`new_password`) | Hidden with the rest of the field menu; the generator stays in the desktop app | Keep it when suggestions are off (it is the same menu under the field the user asked to hide) |

## 3. Manifest

`apps/extension/manifest/base.json`:

```json
"permissions": ["nativeMessaging", "activeTab", "scripting", "storage"],
"host_permissions": ["https://*/*", "http://*/*"]
```

`optional_host_permissions` is removed. `storage` shows no install warning.
Firefox (strict_min_version 128) lists host permissions in the install
prompt and grants them at install; the user can withdraw them later in
`about:addons`, as in Chrome.

Consequences, to be stated in the docs and release notes:

* The store and the install prompt show "Read and change all your data on
  all websites".
* **Chrome disables an installed 0.9.x extension when it updates to this
  version**, until the user accepts the new permission. This is Chrome's
  permission-escalation behavior and cannot be avoided with required host
  permissions.

## 4. Content-script registration

`background/registration.ts` is unchanged in behavior: it still registers
the inline content script and the two passkey scripts for exactly the
granted patterns (`grantedOrigins()`), and re-syncs on
`permissions.onAdded` / `onRemoved`. With required host permissions the
grant is normally complete from install. When the user restricts site
access in the browser (Chrome's "On specific sites" / "On click", Firefox's
per-add-on permissions), the scripts follow the grant as today. The file's
header comment is updated: the scripts are registered by default, not
opt-in.

The content script is registered regardless of the suggestions preference,
because it is also what detects submissions for the save prompt.

## 5. The suggestions preference

New module `apps/extension/src/shared/prefs.ts`:

```ts
export async function getInlineSuggestions(): Promise<boolean>; // absent or unreadable → true
export async function setInlineSuggestions(on: boolean): Promise<void>;
export function onInlineSuggestionsChanged(cb: (on: boolean) => void): void; // storage.onChanged, area "local"
```

Only this boolean is stored. Nothing else goes to extension storage; the
rule "no secrets in extension storage" (`security-model.md`) is unchanged.
A value of any other type is treated as on.

### 5.1 Background (authoritative)

`inline-handler.ts` `openMenu` reads the preference first. When off it
returns `{ ok: false }` (the reply already used when there is nothing to
offer), for every kind and with `explicit` too, before asking the desktop
anything, and opens no frame. Every menu path goes through `openMenu`, so
this is the single place that decides.

Not affected: `cs_submit` → `check_login` → save prompt, `cs_ready`
(reshowing a pending save prompt after navigation), `cs_run_step` (auto
sign-in continuing a run the user started from the popup), passkey
messages.

### 5.2 Content script

`content/index.ts` reads the preference at `start()` and follows
`onInlineSuggestionsChanged`, so no reload is needed. While off:

* `showIcon` does nothing; turning the preference off hides a shown icon
  and closes an open menu.
* `maybeOpen` returns without asking (pointerdown, Tab focus, ArrowDown).

Everything else is unchanged: value-origin tracking, submit/Enter/button
capture, multi-step username memory, generated-password-on-unload, fills
sent by the background, the save prompt frame.

## 6. Options page

`options.html` / `options.ts` get two parts instead of one button:

1. **Suggestions in login fields** — a switch bound to the preference.
   Copy: "Show my logins under login fields". Note: "Saving new or changed
   passwords and passkeys work with this off."
2. **Site access** — a status line from `grantedOrigins()`:
   * all granted: "HavenKeys can offer to save logins and handle passkeys
     on the sites you visit."
   * https only: same, "on secure (https) sites".
   * none (the user withdrew it in the browser): "Site access is off, so
     HavenKeys cannot offer to save logins or handle passkeys. Filling
     from the toolbar button still works." plus an **Allow** button that
     calls `chrome.permissions.request` in the click handler.

The toggle no longer requests or removes host permissions.

## 7. Toolbar popup

The "In-page suggestions are off" notice (`offerSuggestions`) is shown only
when site access is missing (`grantedOrigins()` empty), not when the
preference is off (that is the user's choice). Its copy changes to site
access: title "Site access is off", body "Allow it to get offers to save
logins and to use passkeys with HavenKeys.", button **Allow**. The success
text stays ("Reload open tabs to use it there").

## 8. Strings

New and changed keys in `i18n/en.ts` and `i18n/pt-BR.ts` (the existing
parity tests require both):

* `options.suggestionsTitle`, `options.toggleLabel`, `options.notes`
  rewritten for the preference; new `options.suggestionsNote`,
  `options.accessTitle`, `options.accessOn`, `options.accessHttps`,
  `options.accessOff`, `options.allow`; `statusOff` / `statusOn` /
  `statusHttps` / `turnOn` / `turnOff` / `withoutAfter` removed or reworded.
* `popup.offer.*` reworded for site access.

pt-BR wording: "Mostrar meus logins abaixo dos campos de login";
"Salvar senhas novas ou alteradas e usar chaves de acesso continuam
funcionando com esta opção desligada."; "Acesso aos sites desativado";
"Permitir".

## 9. Security considerations

* **Larger default surface.** The isolated content script and the two
  passkey scripts now run in every http(s) page by default, instead of only
  after the user opts in. They already treat every page as hostile
  (`threat-model.md` §2, T8) and were designed to run everywhere once
  granted; nothing in them changes. What changes is the default exposure to
  a bug in them or to a compromised extension build: it now reaches every
  page for every user. This is accepted as the cost of save prompts and
  passkeys working without setup.
* **No new secret path.** The preference is a non-sensitive boolean. Every
  request still goes through the desktop, which still checks integration
  switch, lock state, origin binding and rate limits in Rust.
* **Desktop integration stays opt-in** (P6 unchanged). Until the user turns
  it on, the scripts run but every page request is refused by the bridge.
* The page cannot read or change the preference: `storage.local` is not
  exposed to page script, and `externally_connectable` stays empty.

## 10. Documentation

* `CLAUDE.md` §18: amendment note pointing to this spec (site access is
  required so save prompts and passkeys work without setup; suggestions
  are a separate preference).
* `docs/security-model.md` §12: permission table (`storage` added; host
  patterns required), the "Why not ask for everything at install"
  paragraph replaced with the reasoning in §9, the "Not requested" list
  (drop `storage`), and the passkey-scripts paragraph ("when the user
  grants" → "on granted hosts, by default all").
* `docs/threat-model.md` §2: "When in-page suggestions are on, a content
  script reads…" → the content script runs on granted hosts, by default
  all.
* `docs/autofill.md`: saving and passkeys do not depend on the
  suggestions preference; the preference hides only the icon and menu.
* `docs/native-messaging.md` §2 step 6: suggestions are on by default;
  the options page can turn them off.
* `docs/security-review.md`: a new informational row for the default-on
  site access, with the accepted trade-off from §9.
* `README.md` (feature table and the "opt-in in-page suggestions" lines)
  and the website copy (`apps/web/src/i18n/en.tsx`, `pt-BR.tsx`,
  `pages/Security.tsx`) where they describe suggestions or site access as
  opt-in.

## 11. Testing

* `manifest.test.ts`: host patterns in `host_permissions`, no
  `optional_host_permissions`, `storage` present, no other new permission.
* `prefs` unit tests: absent → true; stored false → false; non-boolean →
  true; change callback fires.
* `inline-handler.test.ts`: preference off → `cs_open_menu` (plain and
  `explicit`) opens nothing; `cs_submit` with a new password still yields
  an Add prompt and with a changed password an Update prompt; `cs_ready`
  still reshows a pending prompt.
* `content.test.ts`: preference off → no icon on focus, no `cs_open_menu`
  on click/Tab/ArrowDown; a form submit still sends `cs_submit`; turning
  the preference off while the menu is open closes it and hides the icon.
* Options page: the switch writes the preference and does not call
  `permissions.request`/`remove`; the Allow button appears only with no
  grant.
* Popup: the site-access notice appears only with no grant, not when the
  preference is off.
* i18n parity tests pass for the new keys.
* Manual: load the unpacked Chrome and Firefox builds fresh; confirm the
  save prompt and passkey card without touching the options page; turn
  suggestions off and confirm the icon/menu are gone while save/update and
  passkeys still work.

## 12. Out of scope

* Changing the desktop `browser_integration` default.
* Per-site suggestion preferences.
* Anything that would let the preference affect Rust-side authorization.
