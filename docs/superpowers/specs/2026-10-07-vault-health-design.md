# Vault health — design

Date: 2026-10-07. Status: approved in conversation.

## 1. Goal

Show the user which logins need attention, and why:

* **Weak** passwords.
* **Reused** passwords (the same password on two or more logins).
* **Old** passwords (not changed in over a year).
* **Passkey available**: the site accepts passkeys and the login has none.
* **2FA available**: the site offers TOTP codes and the login has no TOTP.

Everything is computed in the Rust core with the vault unlocked. Nothing
leaves the device, and no third-party service is contacted. Desktop and
Android ship it together, over the same Rust API.

## 2. Decisions taken

| Question | Decision |
|---|---|
| Platforms | Desktop and Android in the same release; the extension gets nothing new |
| Checks | All five: weak, reused, old, passkey available, 2FA available |
| Where it is computed | On demand in `havenkeys-core` (approach A); per-item flags stored at save time (B) and computing in the UI (C) rejected |
| Weak | `zxcvbn` score 0–2, with the login's title and username as user inputs |
| Reused | Current passwords only; `password_history` is not compared |
| Old | Over 365 days since the password last changed |
| Dismissing | Per login and per check, stored in the encrypted item, synced |
| Site directories | Bundled in the core, refreshed by script and reviewed like code; never fetched at runtime |
| Leaked passwords (Have I Been Pwned) | Out of scope. A later option, off by default, would need its own spec |

## 3. No CLAUDE.md amendment

This feature does not relax any rule. It reads the unlocked vault inside
Rust (§2), returns no secret to JavaScript or Kotlin (§33), adds no
network access (the help links open in the system browser on an explicit
click, like a login's website does today) and gives the extension nothing
new. The dismissal field is ordinary encrypted item data (§36).

## 4. The checks (`crates/havenkeys-core/src/health/`)

Only `login` items are checked. Cards, identities and secure notes are not.

### 4.1 Weak

* `zxcvbn::zxcvbn(password, &[title, username])`; score 0, 1 or 2 is weak.
* The user inputs make a password built from the site or account name
  score low (e.g. `github2024` on a login titled "GitHub").
* A login with no password (passkey only) is not checked.
* Cost is a few milliseconds per password; the whole report runs on a
  worker thread (§5.3).

### 4.2 Reused

* Logins whose current passwords are equal (constant-time comparison is
  not needed: both sides are already in the process). Grouping uses a
  `HashMap` keyed by a borrowed `&str` of the password, which is dropped
  when the report is built; nothing derived from the password is kept.
* Every group of two or more logins becomes one `reused_group` number.
* Empty passwords are ignored. `password_history` is not compared.

### 4.3 Old

* Last change = `password_history[0].replaced_at` if the history is not
  empty, else `created_at`.
* Old when `now - last_change > 365 days`.
* Known limitation: an imported login's `created_at` is the import time
  (unless the import format carried one), so it looks newer than it is.

### 4.4 Passkey available

* A login is flagged when one of its URL rules matches a site in the
  passkey directory and the login holds no passkey.
* Matching uses `origin.rs`: the rule's host must equal a directory domain
  or be a subdomain of it, decided on the registrable domain from the
  Public Suffix List. Never string matching. `evilgithub.com` and
  `github.com.evil.com` never match `github.com`.
* A directory entry counts only when `passwordless` is true (the site lets
  a passkey replace the password).

### 4.5 2FA available

* Flagged when a URL rule matches a site in the two-factor directory
  (sites listing `totp`), the login has no TOTP, and the login is **not**
  already flagged "passkey available". One suggestion per site.

### 4.6 Dismissed checks

* `ItemDetails::Login` gains
  `#[serde(default, skip_serializing_if = "Vec::is_empty")] health_ignored: Vec<HealthCheck>`.
  `HealthCheck` is `weak | reused | old | passkey | two_factor`.
* A dismissed check is reported under `dismissed`, not under `issues`, and
  is not counted.
* No migration: the vault is resettable and the field defaults to empty.
* Every existing item editor keeps the field unchanged, the same way
  `app_bindings` is kept today.

## 5. Interface

### 5.1 Report

```text
HealthReport {
  computed_at: i64,
  counts: { weak, reused, old, passkey, two_factor },   // non-dismissed only
  issues:    [ { item_id, checks: [HealthCheck], reused_group: Option<u32> } ],
  dismissed: [ { item_id, checks: [HealthCheck] } ],
}
```

* Only item IDs, check kinds and group numbers. No password, score,
  length, hash or any other value derived from a password. No title or
  username either: both UIs already hold the item overviews.
* `reused_group` is an index (0, 1, 2…) assigned when the report is built,
  in an order unrelated to the password.
* `Debug` on every report type is redacted, like the other model types.

### 5.2 Operations

| Core | Desktop (Tauri, allowlisted) | Android (`havenkeys-mobile`) |
|---|---|---|
| `health_report() -> HealthReport` | `health_report` | `healthReport()` |
| `set_health_ignored(item_id, checks)` | `set_health_ignored` | `setHealthIgnored()` |
| `health_help_url(item_id, check) -> Url` | `open_health_help` (Rust opens it) | `healthHelpUrl()`; Kotlin hands it to an `Intent` unchanged |

* `set_health_ignored` is an item edit through the existing server-first
  write path, so it needs the server, like every change. It does **not**
  change `updated_at`: dismissing must not move the item up in "recently
  edited", and it does not change the password's age (§4.3 does not read
  `updated_at` anyway). `item_id` must be a login; anything else is
  rejected.
* `health_help_url` looks the site up in the directory itself from the
  login's own URL rules; the UI never passes a URL. Only `https` help
  links from the directory are returned; no link → `NotFound`.
* A locked vault → `Locked`. Errors never carry item contents.
* Native messaging is not touched.

### 5.3 Caching

* The report is cached in the unlocked vault state. Any write, a sync pull
  and unlock clear it; lock drops it with the rest of the decrypted state.
* The next call computes it on a worker thread (desktop: Tauri async
  command; Android: the coroutine the ViewModel already uses for core
  calls). The UI shows a spinner, never a partial report.

## 6. Site directories

* `apps/extension/src/data/passkey-sites.json` moves to
  `crates/havenkeys-core/data/passkey-sites.json`. The extension imports it
  from there (one copy). `scripts/update-passkey-directory.mjs` writes the
  new path.
* New `crates/havenkeys-core/data/twofactor-sites.json`, built by
  `scripts/update-twofactor-directory.mjs` from
  `2factorauth/twofactorauth` (`entries/*/*.json`): only entries whose
  `tfa` list contains `totp`; fields `{ name, domains, help }` with the
  same hostname, name and https-URL cleaning as the passkey script.
  Expected size 50–100 KB.
* Before the data is committed, the twofactorauth repository's license is
  checked and recorded in the third-party notices with the commit used,
  as for the passkey directory (CC-BY-4.0).
* Rust embeds both with `include_str!` and parses them once (lazily). An
  entry that fails validation is dropped, never a panic. Note: in the
  passkey JSON, `mfa: true` means "a passkey can be the second factor",
  not TOTP; the 2FA check uses only the new directory.

## 7. Desktop

* **Sidebar:** "Vault health" under Tools, next to the Generator, with a
  badge counting non-dismissed issues (`nav-count`).
* **View:** a row of five summary cards (Weak · Reused · Old · Passkey
  available · 2FA available) with counts; clicking one filters the list.
  A "Dismissed" filter lists dismissed checks with "Undo".
* **Rows:** title and username from the overviews, check chips, and actions:
  * **Open**: the login's detail.
  * **Change password**: the login's editor with the generator ready.
  * **How to enable** (passkey and 2FA only): `open_health_help`.
  * **Dismiss** (per check).
* **Login detail:** the same chips, under the title.
* **Empty state:** "No issues found." Never "Your vault is secure."
* **Copy:** "Weak password", "Used in 3 logins", "Not changed in over a
  year", "Supports passkeys", "Supports two-factor codes". English and
  pt-BR.

## 8. Android

* **Home:** one "Vault health" card with the total, opening the Health
  screen.
* **Health screen:** the same five filters and "Dismissed", rows with Open,
  Change password, How to enable and Dismiss.
* **Item screen:** the same chips.
* English and pt-BR strings.

## 9. Security analysis

* **Secret exposure:** the report holds no secret and nothing derived from
  one (§5.1). Passwords are read inside Rust, as for search and fill. The
  grouping map borrows the passwords and is dropped before the report is
  returned.
* **What the report itself reveals:** which logins are weak or reused. Any
  attacker able to read the UI's memory can already read the overviews
  and request reveals; the report lives only in the unlocked state and is
  dropped on lock.
* **Directory data:** hostile-input parsing (§6), fuzzed. A poisoned
  directory could at worst suggest a wrong help link; links are https only
  and come from reviewed, committed data.
* **Help links:** opened only on an explicit click, in the system browser;
  the desktop never loads a remote page (§41).
* **Dismissal:** ordinary encrypted item data; the server sees one more
  ciphertext update, as for any edit.
* **Dependency:** `zxcvbn` (Rust port, MIT) is checked for maintenance and
  passes `cargo audit` / `cargo deny` before it is added (§48). It adds
  about 1 MB of dictionaries; the before/after size of the desktop binary
  and the Android `.so` is recorded.

## 10. Testing

* **Weak:** weak flagged; a generated 24-character password not flagged; a
  password built from the title flagged.
* **Reused:** groups form; history-only matches ignored; empty passwords
  ignored.
* **Old:** age from `replaced_at` and from `created_at`; boundary at
  exactly 365 days.
* **Passkey / 2FA:** registrable-domain matches; `evilgithub.com`,
  `github.com.evil.com` and `github-login.example.com` never match; logins
  with a passkey / TOTP not flagged; a site in both directories gets only
  the passkey suggestion.
* **Dismissed:** moves to `dismissed`, leaves the counts, leaves
  `updated_at` alone, survives an edit from every editor; non-login IDs
  rejected.
* **Locked:** every operation returns `Locked`.
* **Cache:** cleared by a write, a sync pull, unlock and lock.
* **Leak test:** the serialized report contains no password or substring
  of one (substrings of 4+ characters).
* **Fuzz:** both directory parsers and their hostname validation.
* **Desktop:** view tests (filters, dismiss and undo, empty state), i18n
  parity, Tauri capability test for the three commands.
* **Android:** ViewModel test; i18n parity.
* **Extension:** existing passkey-site tests pass with the moved JSON.

## 11. Documentation

* `docs/security-model.md`: what the health report contains and omits,
  where it lives and when it is dropped.
* `docs/security-review.md`: a new entry for this feature.
* `docs/architecture.md`: the `health` module and the bundled directories.
* Third-party notices: twofactorauth data and `zxcvbn`.
* `docs/ideas.md`: mark item 1 as specified, and correct its note: the
  passkey list's `mfa` field is not TOTP; TOTP sites come from
  `2factorauth/twofactorauth`.
