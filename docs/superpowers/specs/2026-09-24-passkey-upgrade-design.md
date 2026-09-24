# Passkey upgrade — Design

Status: proposed, 2026-09-24.
Builds on `2026-09-23-passkeys-design.md` (the passkeys spec) and amends its
§5.3 rule that nothing is created without a click in HavenKeys UI (see §3.4).

> This software has not undergone an independent security audit.

## 1. Goal

Help the user move accounts onto HavenKeys passkeys with little per-site
effort:

1. **Silent upgrade.** Right after HavenKeys fills a password and the user
   signs in, a site that asks for a passkey (`navigator.credentials.create`
   with `mediation: "conditional"`, the "automatic passkey upgrade") gets one
   from HavenKeys, attached to the login that was just filled.
2. **Site list hint.** On sites known to support passkeys where the user has
   a saved password but no passkey, the field menu offers a link to the
   site's passkey help article.
3. **Passkeys first.** When the user has a passkey for the site, the menu
   leads with it: passkey rows when the site offers passkey autofill,
   otherwise a hint to use the site's passkey sign-in.

HavenKeys cannot register a passkey on its own: a relying party must issue
the challenge and store the public key. Every path above therefore depends on
the site calling WebAuthn; HavenKeys only answers, suggests, or points.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Consent for a silent passkey | A HavenKeys password fill on the same site in the last 5 minutes (browser practice) | Opt-in only; always a card |
| Can the user turn it off | Yes: vault setting `auto_passkey_upgrade`, default on; when off, the save card asks instead | No setting |
| Undo on the "saved" notice | No; the notice points to the desktop app | Undo (the site keeps the public key anyway; needs extension-side delete) |
| Site list source | 2factorauth Passkeys Directory (CC-BY-4.0), snapshot committed to the repo | passkeys.directory (1Password; no published data or license); hand-curated list; runtime API |
| Where "add one" leads | The entry's `documentation` URL (help article) in a new tab | A settings-page deep link (not in the data) |
| "Don't show again" | Not in this version (no browser storage allowed in the extension) | Per-site dismissal |

## 3. Silent upgrade

### 3.1 Page script

`page.ts` currently hands `create({ mediation: "conditional" })` straight to
the browser. It now forwards it like any other create, with
`CreateOptions.conditional = true`. All existing fallback rules still apply:
anything unsupported, refused or failing ends in the browser's original
`create()`.

### 3.2 Recent fills (Rust core)

`VaultService` keeps, inside the unlocked `Session`, a small in-memory list of
recent password fills: `(item_id, site, filled_at_ms)`, where `site` is the
registrable domain of the page URL the browser reported (or the host when it
has none). It is recorded by `fill_for_page` only when a password was
returned. It keeps at most 16 entries, drops entries older than
`UPGRADE_WINDOW_MS = 5 * 60_000`, is never persisted, and is dropped with the
session on lock.

### 3.3 Checks (Rust core)

`check_passkey_create` gains an `upgrade` field in its result, computed only
for conditional requests:

```text
upgrade: none | ask{item_id} | auto{item_id}
```

It is `ask` or `auto` only when all of these hold; otherwise `none`:

* `authorize_rp(rp_id, url, top_url)` succeeds (unchanged);
* a recent fill exists whose `site` equals the page's site and whose login is
  still offered for the page by `find_matches`;
* the request's `userName`, folded (trim, lowercase), equals that login's
  username, or the login has no username;
* the login can take another passkey (limit, as today).

`auto` when the vault setting `auto_passkey_upgrade` is on, `ask` when it is
off. `excluded` handling is unchanged and takes precedence.

`passkey_create` gains `conditional: bool`. When true, Rust recomputes the
upgrade decision and refuses (`Denied`) unless it is `auto` for exactly the
requested `itemId`. So the extension alone can never save a passkey
silently. When false, behavior is unchanged (the save card was clicked).

### 3.4 Extension flow and the amended rule

For a conditional create the background calls `check_passkey_create`:

* `upgrade: auto` → `passkey_create { conditional: true, itemId }` without
  showing a card. On success the bridge shows a notice frame for 4 seconds:
  "Passkey saved to HavenKeys · <site>", "Manage it in the HavenKeys app".
  The credential is returned to the site.
* `upgrade: ask` → the existing save card, titled "Add a passkey?", with the
  filled login preselected. A click is required, as today.
* `upgrade: none`, `excluded`, locked, offline or any error → fallback to the
  browser (which normally does nothing for a conditional create).

Amendment to the passkeys spec §5.3: a passkey may be created without a click
in HavenKeys UI only through `upgrade: auto`, whose consent is the user's
password-fill click on the same site within the previous 5 minutes, confirmed
in Rust. A compromised extension can at most add a passkey to the login the
user just filled on that site, within that window.

### 3.5 Setting

`Settings.auto_passkey_upgrade: bool`, `#[serde(default = "true")]`, saved in
the encrypted vault settings like `browser_integration`. The desktop Settings
screen gets a toggle: "Add passkeys automatically after I sign in", with the
note "When off, HavenKeys asks first."

## 4. Site list hint

### 4.1 Data

`scripts/update-passkey-directory` (run by hand, not at build time) fetches the
2factorauth Passkeys Directory API and writes
`apps/extension/src/data/passkey-sites.json`:

```text
[{ name, domains: [primary, ...additional], passwordless: bool, mfa: bool, help: https-url | null }]
```

The script drops entries without a valid hostname, and keeps `help` only when
it is an `https:` URL. The file is committed and reviewed like code; the
extension never fetches it. Attribution (CC-BY-4.0, "Passkeys Directory by
2factorauth") goes in `THIRD-PARTY-NOTICES.md` and `docs/autofill.md`. A unit
test validates the committed file's shape.

### 4.2 Matching

In the background worker, a page matches an entry when its hostname equals
one of the entry's domains or ends with `"." + domain`. It is only a UI hint,
so no PSL and no Rust check are needed; a mismatch can show or hide a help
link, nothing more. `github.com.evil.com` never matches `github.com`.

### 4.3 Knowing whether a passkey exists

New native request `passkey_status { url, topUrl? }` → `{ hasPasskey: bool }`.
Rust answers true when any passkey is bound to an rpId that `authorize_rp`
allows for the page, and returns nothing else. It is a Lookup-class request.
The result is kept only for the open menu session.

### 4.4 The row

Shown last in a login field menu, only when that menu already lists at
least one password login for the site, the page matches the list, and
`hasPasskey` is false. Text: "<name> supports passkeys — how to add one". Its
click goes through the menu's trusted-click guard. The background then opens
`help` with `chrome.tabs.create` (no new permission) and closes the menu. With
no `help` URL, the row is not shown.

## 5. Passkeys first

* The site offers passkey autofill (a conditional `get()` is waiting):
  unchanged. Passkey rows come first.
* Otherwise, when `hasPasskey` is true: the first row is a non-interactive
  hint, "You have a passkey for <site> — use the site's 'Sign in with a
  passkey' option", followed by the password rows. It has no click action.
* Otherwise the menu is unchanged.

## 6. Protocol changes

| Message | Change |
|---|---|
| `check_passkey_create` | request gains `conditional: bool`; result gains `upgrade: {kind: "none"} \| {kind: "ask", itemId} \| {kind: "auto", itemId}` (only `none` when `conditional` is false) |
| `passkey_create` | request gains `conditional: bool` |
| `passkey_status` | new; `{url, topUrl?}` → `{hasPasskey}` |

All new fields use `deny_unknown_fields` and exact-key parsing on both sides.
No response carries a key, a credential ID beyond what exists today, or a
secret.

## 7. Errors and states

| Situation | Behavior |
|---|---|
| Conditional create, no recent fill / name mismatch / setting on but login full | Fallback, silently |
| Conditional create, setting off, recent fill | "Add a passkey?" card |
| Silent save offline or failing | Fallback, no notice |
| Vault locks within the window | Fill memory gone; later conditional creates fall back |
| `passkey_status` fails (locked, desktop gone) | Treated as false; no hint, no site-list row |

## 8. Security analysis (additions to `threat-model.md`)

* **Relaxed click rule.** Covered in §3.4. The residual risk is a passkey
  added to the just-filled login without an explicit HavenKeys click. The
  user can turn it off.
* **Fill memory.** Item IDs and sites only, in the unlocked session, never
  persisted, cleared on lock.
* **`passkey_status` oracle.** Tells a granted page whether HavenKeys holds a
  passkey for its own rpId. It is asked only when the user opens a field menu,
  and the answer never reaches the page (it only changes our menu, which the
  page cannot read). A page can see the menu frame's size change. That is
  recorded as a residual signal, like the existing menu-presence signal.
* **Directory data.** Untrusted third-party text. It is shown via
  `textContent` only, links are https-only, the file is reviewed on update,
  and it is never fetched at runtime.

## 9. Testing

**Rust:** the upgrade decision is `auto`/`ask`/`none` for each rule in §3.3.
`passkey_create {conditional:true}` is refused without a matching `auto`, for
another item, after 5 minutes, and with the setting off. Fill memory is
cleared on lock. `passkey_status` does not see other sites' passkeys.
A1–A3 are re-run on the new paths. Protocol parse and bounds tests cover the
new fields.

**Extension:** conditional create routes to auto (notice), ask (card) or
fallback. Directory matching, including the evil-suffix cases. Menu ordering
and the hint row. The help link opens only on a trusted click. The
committed `passkey-sites.json` validates. The hygiene tests still pass.

**Desktop:** the setting toggle round-trips.

**Manual:** Google's automatic upgrade in Chrome with the setting on and off,
and a directory site showing the help row.
