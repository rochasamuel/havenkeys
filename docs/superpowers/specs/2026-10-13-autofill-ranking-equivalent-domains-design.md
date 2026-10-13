# Autofill ranking and equivalent domains — design

Date: 2026-10-13. Status: approved in conversation.

Implements items **B1** (frecency ranking) and **B2** (equivalent domains)
of `docs/autofill-roadmap.md`. B3 (sign-in URL hints, "never fill") was
considered and dropped: an `Exact` website rule already covers sign-in URL
hints, and B1 removes most of the menu clutter "never fill" was for.

## 1. Goal

* **Better order.** Among logins that match a page equally well, the one
  the user fills most (recently) comes first.
* **Wider reach, conservatively.** A login saved for `apple.com` is offered
  on `icloud.com`, from a short, compiled, reviewed list of domains that one
  company owns and that sign in with the same account.

Everything happens in the Rust core, offline. No telemetry, no network, no
change to how the extension or Android display suggestions (both keep the
order Rust returns).

## 2. Decisions taken

| Question | Decision |
|---|---|
| Ranking key | `(strength, −decayed use score, lowercase title, id)`; strength always first |
| Usage data | The existing sealed, per-device, never-synced activity record (`activity.rs`) |
| What counts as a use | Every password `fill_for_page` returns, plus the uses Android already records |
| Equivalence source | Compiled `crates/havenkeys-core/data/equivalent-domains.json`, curated by us (no AGPL lists copied) |
| Which rules widen | `Domain` rules only; `Exact` and `Origin` stay as narrow as the user set them |
| Strength of a widened match | New `MatchStrength::Equivalent`, weakest |
| Default | On; the per-device setting `equivalent_domains` (not synced) turns it off; `Settings::restrictive()` has it off |
| UI | One switch in desktop Settings and one in Android Settings; impeccable review for both |
| Passkeys | Unaffected (rpId rules are separate) |
| User-defined groups | Out of scope |

## 3. B1 — frecency ranking

### 3.1 Recording

The desktop bridge's `fill_item` arm calls `record_use(id, now_ms)` after
the fill returned a password, next to the existing `record_fill`. It is not
in `VaultService::fill_for_page`, because Android calls `fill_for_page`
before the user's pick (to build the datasets handed to the autofill
framework), so recording there would count offers as uses. Android records
its own picks (and copies) through `havenkeys-mobile`. A failure to record
is ignored: the fill is the point, the activity a convenience (same rule as
`MobileVault::note_use`).

The desktop thereby adopts the activity record, as the 2026-10-03
amendment to `CLAUDE.md` foresaw ("the desktop may adopt it later"). No
desktop UI reads it yet; only ranking does.

### 3.2 Ranking

`find_matches(page_url, top_url, now_ms)` and
`matches_for_app(app, verified_hosts, now_ms)` gain `now_ms`. They read the
activity record once per call and sort by:

1. `strength` (stronger first),
2. decayed score at `now_ms` (higher first; no record = 0),
3. lowercased title, then id (today's tie-break).

Because strength is first, usage only reorders logins that match equally
well; a frequently used `SameSite` login never outranks an `ExactUrl` one.

An unreadable activity record reads as empty (existing `activity()`
behaviour), so ranking falls back to today's order and never fails the
request. `Error::Locked` still propagates.

Callers updated: `havenkeys-bridge` dispatch (three `find_matches` call
sites), `havenkeys-mobile` `autofill.rs` and `credentials.rs`, and the
`matches_for_app` callers.

### 3.3 Privacy

Unchanged from the activity spec: per device, sealed under the vault key
in a `LocalSlot`, never synced, uses older than a year forgotten, deleted
items dropped. The server learns nothing new.

## 4. B2 — equivalent domains

### 4.1 Data

`crates/havenkeys-core/data/equivalent-domains.json`:

```json
{
  "groups": [
    { "note": "Apple Account", "domains": ["apple.com", "icloud.com"] }
  ]
}
```

* A group lists registrable domains (eTLD+1) that **one company owns and
  that sign in with the same account**. `note` says which account; it is
  for reviewers and is not shown to users.
* The first list is small (10–20 groups) and conservative: when in doubt,
  leave it out. A domain whose login is separate (for example a regional
  store with its own accounts) is not in a group.
* Loaded with `include_str!` and parsed once (`OnceLock`), like
  `passkey-sites.json`. A table that fails to parse is treated as empty: it
  can only narrow, never widen.

Table checks, as a unit test that runs in CI:

* every entry is lowercase ASCII (punycode), equals
  `registrable_domain_of(entry)` and has a known public suffix;
* no domain appears in two groups;
* every group has at least two domains and a non-empty note.

### 4.2 Matching

In `origin.rs`:

* `pub struct MatchPolicy { pub equivalent_domains: bool }`.
* `match_rule(rule, page, policy)` and `match_item(item, page, policy)`
  take it.
* After the existing `Domain` branch finds no same-site match: if the
  policy allows, both registrable domains exist, differ, and are in the
  same group, the result is `MatchStrength::Equivalent`.
* The scheme (`https` rule never matches `http` page) and port checks run
  first, unchanged.
* `MatchStrength::Equivalent` sorts after `SameSite`.

`PageContext::matches` passes the session's policy to both checks; a frame
must match the item and so must the top page, either of which may match
through a group.

### 4.3 Where the policy applies

Through `PageContext` and `app_match`: `find_matches`, `fill_for_page`,
`totp_for_page` (so auto-sign-in, which runs only after those authorize),
and Android `matches_for_app` (asset-links verified hosts).

### 4.4 Where it does not

* Passkeys: the relying-party ID check in `passkey` code is untouched.
* The "Sign in with" provider-origin list.
* `same_site` / `site_of` users: recent-fill records and the
  passkey-upgrade consent window keep exact sites.
* Save-login deduplication (`check_login`, `check_login_for_app`,
  `check_sso`) compares the direct matches first. Only when they give
  `Add` are the setting-following matches compared, and only to return
  `Unchanged` (the login was just filled there, so no prompt); never
  `Update`. Revised in the final review: without this, a login filled on an
  equivalent site was proposed again as new on every sign-in.
* Save-update authorization (page, SSO and app) stays direct
  (`MatchPolicy::DIRECT`): a tampered extension cannot overwrite a password
  from an equivalent site.

### 4.5 Setting

`Settings::equivalent_domains: bool`, `#[serde(default = "default_true")]`.
`Settings::restrictive()` sets it `false`, so a damaged or tampered
settings blob narrows matching. Like the other settings, it is per
device: a sealed row in the local SQLite store that is never sent to the
server, so each device has its own switch.

* Desktop: a switch in Settings, with the other autofill switches
  (`SettingsView.tsx`, `i18n/en.ts`, `i18n/pt-BR.ts`, `lib/types.ts`).
* Android: `MobileSettings` gains `equivalent_domains`, written into the
  shared settings by `update_settings`; a switch in the Settings screen.

Label (en): "Suggest logins on related sites", description: "Suggests a
login saved for one site on another site of the same company, such as an
apple.com login on icloud.com. You still choose it yourself." (pt-BR:
"Sugerir logins em sites relacionados".) Final copy from the impeccable
review; the first draft said "Fill on related sites".

## 5. CLAUDE.md amendment

> Amended on 2026-10-13 by
> `docs/superpowers/specs/2026-10-13-autofill-ranking-equivalent-domains-design.md`:
> a login with a "domain" website rule also matches pages on domains that a
> fixed, reviewed list in Rust records as owned by the same company and
> sharing its sign-in (for example `icloud.com` for `apple.com`), ranked
> below direct matches. A vault setting turns this off.

Placed after the §32 / §24 text it relaxes.

## 6. Security review entry

New `security-review.md` entry:

* **Risk:** a wrong group, or a domain in a group later changing owner,
  offers a login to a site that does not own it.
* **Mitigations:** `Domain` rules only; explicit user pick still required
  (§25); list compiled in and changed only by code review with a `note` per
  group; table checks in CI; weakest strength; setting is a kill switch;
  restrictive settings turn it off; no network.
* **Remaining:** the list can go stale between releases.

## 7. Tests

Core (`origin.rs`, `vault.rs`, `activity.rs`):

* `apple.com` `Domain` rule on `https://www.icloud.com/` → `Equivalent`.
* Same with `Exact` / `Origin` rule → no match; policy off → no match.
* `icloud.com.evil.com`, `evil-icloud.com`, `icloud.co` → no match.
* `https` rule on `http://icloud.com/` → no match.
* A frame on `icloud.com` inside an unrelated top page → no match.
* Table checks (§4.1).
* Ranking: higher use score first among equal strength; never above a
  stronger match; decay changes the order over time; damaged activity
  record → today's order; locked → `Locked`.
* `fill_for_page` records a use; a denied fill does not.
* `Settings` deserialised without the field → on; `restrictive()` → off.

Bridge (`dispatch.rs`): `fill_item` from an equivalent site succeeds;
with the setting off it is denied; `find_matches` order follows usage.

Mobile: settings round-trip of `equivalent_domains`.

## 8. Docs

`docs/autofill.md` (matching rules and ranking), `docs/security-model.md`
(matching), `docs/autofill-roadmap.md` (B1, B2 done; B3 dropped with the
reason), `security-review.md` (§6), `CLAUDE.md` (§5).
