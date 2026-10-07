# Vault Health Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Watchtower-style "Vault health" report (weak, reused, old, unsecured website, duplicates, passkey available, 2FA available) computed in the Rust core and shown on desktop and Android, with per-login dismissals that sync.

**Architecture:** A new `havenkeys-core::health` module: two bundled site directories, a pure `compute(snapshot, now)` and `VaultService` methods that take a snapshot under the vault lock, cache the report in the session and stage dismissals. `HavenClient` runs the slow part (zxcvbn) outside the vault lock and pushes dismissals. Desktop (Tauri commands + React view) and Android (UniFFI + Compose screen) only render IDs, check kinds and counts.

**Tech Stack:** Rust (`zxcvbn` 3.1, `psl`, `url`, `serde`), Tauri 2, React + TypeScript + Vitest, UniFFI, Kotlin + Jetpack Compose, Node scripts for the directories.

**Spec:** `docs/superpowers/specs/2026-10-07-vault-health-design.md`

## Global Constraints

- Only `login` items are checked; cards, identities and notes never appear in the report.
- The report carries item IDs, check kinds, `reused_group` / `duplicate_group` indexes and counts. Never a password, score, length, hash, title, username or URL.
- Weak = zxcvbn score 0, 1 or 2, with the login's title and username as user inputs.
- Reused = equal current passwords on two or more logins; `password_history` is not compared; empty passwords ignored.
- Old = more than 365 days (`365 * 24 * 3600 * 1000` ms) since `password_history[0].replaced_at`, else `created_at`; only logins with a password.
- Passkey available = a URL rule's host is a directory domain or its subdomain (walking labels down to the registrable domain only), directory entry has `passwordless: true`, login has no passkey.
- 2FA available = same matching against the TOTP directory, login has no TOTP, and the login is not already flagged "passkey available".
- Unsecured = an `http` URL rule whose host has a registrable domain under a known public suffix (no `localhost`, IPs, `router.lan`).
- Duplicates = logins with equal `dedupe_key_parts(overview, None)` digests (title, username, sorted URLs).
- Group indexes are assigned in order of the smallest item ID (`Uuid` order) in each group.
- Dismissals: `health_ignored: Vec<HealthCheck>` in `ItemDetails::Login`, `#[serde(default, skip_serializing_if = "Vec::is_empty")]`; dismissing does **not** change `updated_at`; every editor keeps the field.
- `HealthCheck` serializes as `weak | reused | old | passkey | two_factor | insecure | duplicate`.
- Cache: in the session; invalid after any write, sync pull, lock or after 10 minutes; a report from an older snapshot is not cached.
- Help links: https only, looked up by Rust from the login's own URL rules; the UI never passes a URL.
- No new native-messaging request; the extension only changes its JSON import path.
- Copy: factual, never "your vault is secure". English and pt-BR on both apps.
- UI follows `apps/desktop/DESIGN.md` and `apps/android/DESIGN.md`, and every UI task ends with an `impeccable` design review against them.
- Commits: no `Co-Authored-By` trailer (owner's rule).

## Review Focus

1. **A vault with hundreds of logins while the extension asks for a fill.** The fill must not wait for zxcvbn: the slow part runs after the vault lock is released (Task 5 test `fill_is_not_blocked_while_health_computes`).
2. **A login with an IDN or a trailing-dot host (`github.com.`) or an uppercase URL.** Matching goes through `host_key` (punycode, dot stripped), so these still match the directory (Task 2 test `lookup_normalizes_hosts`).
3. **The vault locks while the report is being computed.** The finished report must not be cached into the new session or returned as if fresh after a relock (Task 4 test `a_report_from_before_a_lock_is_not_cached`).
4. **A dismissed check on a login that is then edited on another device** (desktop editor, Android editor, browser save, passkey save): the dismissal must survive (Task 3 test `every_edit_path_keeps_health_ignored`).
5. **Huge passwords (10 000 characters) and passwords with emoji.** zxcvbn truncates to 100 chars; no panic, no quadratic slowdown (Task 3 test `long_and_unicode_passwords_do_not_panic`).

---

## File structure

**Core (`crates/havenkeys-core`)**
- Create `data/passkey-sites.json` (moved), `data/twofactor-sites.json` (new).
- Create `src/health/mod.rs`: public types (`HealthCheck`, `HealthCounts`, `HealthIssue`, `HealthDismissed`, `HealthReport`, `HealthSnapshot`) and `compute`.
- Create `src/health/directory.rs`: parse and look up the two directories.
- Create `src/health/vault.rs`: `impl VaultService` (snapshot, cache, stage dismissal, help URL).
- Modify `src/lib.rs`, `src/model.rs` (field), `src/vault.rs` (field carry, session cache, generation bumps, `dedupe_key_parts` visibility), `src/sync.rs` (generation bump), plus every `ItemDetails::Login { .. }` constructor the compiler flags.

**Client (`crates/havenkeys-client`)**: create `src/health.rs`.

**Desktop**: `src-tauri/src/health.rs`, `build.rs`, `src/lib.rs`, `capabilities/main.json`; `src/lib/api.ts`, `src/lib/types.ts`, `src/lib/health.ts` (+ test), `src/views/HealthView.tsx` (+ test), `src/components/HealthChips.tsx`, `src/views/VaultScreen.tsx`, `src/views/ItemDetail.tsx`, `src/i18n/en.ts`, `src/i18n/pt-BR.ts`, `src/styles.css`.

**Mobile FFI**: `crates/havenkeys-mobile/src/health.rs`, `src/lib.rs`, test in `crates/havenkeys-mobile/tests/`.

**Android**: `data/VaultRepository.kt`, `ui/health/HealthViewModel.kt`, `ui/health/HealthScreen.kt`, `ui/health/HealthChips.kt`, `ui/home/HomeViewModel.kt`, `ui/home/HomeScreen.kt`, `ui/item/ItemViewModel.kt`, `ui/item/ItemScreen.kt`, `ui/nav/Routes.kt`, `ui/nav/HavenNavHost.kt`, `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`, tests and the fake repository.

**Scripts/extension/docs**: `scripts/update-passkey-directory.mjs`, `scripts/update-twofactor-directory.mjs`, `apps/extension/src/background/passkey-sites.ts` (+ test import), `THIRD-PARTY-NOTICES.md`, `docs/security-model.md`, `docs/architecture.md`, `docs/security-review.md`, `docs/ideas.md`.

---

### Task 1: Site directories (data, scripts, extension import)

**Files:**
- Move: `apps/extension/src/data/passkey-sites.json` → `crates/havenkeys-core/data/passkey-sites.json`
- Modify: `scripts/update-passkey-directory.mjs` (the `OUT` constant)
- Create: `scripts/update-twofactor-directory.mjs`
- Create: `crates/havenkeys-core/data/twofactor-sites.json` (generated)
- Modify: `apps/extension/src/background/passkey-sites.ts:10`, `apps/extension/src/background/passkey-sites.test.ts:2`
- Modify: `THIRD-PARTY-NOTICES.md`

**Interfaces:**
- Produces: `crates/havenkeys-core/data/passkey-sites.json` (unchanged shape `{name, domains, passwordless, mfa, help}`) and `crates/havenkeys-core/data/twofactor-sites.json` (`[{ "name": string, "domains": string[], "help": string | null }]`, sorted like the passkey file).

- [ ] **Step 1: Move the passkey JSON and point the script and the extension at it**

```bash
git mv apps/extension/src/data/passkey-sites.json crates/havenkeys-core/data/passkey-sites.json
```

In `scripts/update-passkey-directory.mjs` replace the `OUT` line:

```js
const OUT = join(dirname(fileURLToPath(import.meta.url)), "../crates/havenkeys-core/data/passkey-sites.json");
```

and the header comment's first line to `// Rebuilds crates/havenkeys-core/data/passkey-sites.json from the Passkeys`.

In `apps/extension/src/background/passkey-sites.ts` and `passkey-sites.test.ts`:

```ts
import raw from "../../../../crates/havenkeys-core/data/passkey-sites.json";
```

Update the comment in `passkey-sites.ts` that says "A snapshot committed with the extension" to "A snapshot committed in the core crate (shared with Vault health)".

- [ ] **Step 2: Check the extension still type-checks, tests and builds**

Run: `pnpm --filter @havenkeys/extension typecheck && pnpm --filter @havenkeys/extension test && pnpm build:extension`
Expected: all pass; `apps/extension/dist/chrome` builds. If `tsc` complains that the file is outside `include`, add `"../../crates/havenkeys-core/data/*.json"` to `include` in `apps/extension/tsconfig.json`. If `src/hygiene.test.ts` scans for JSON under `src/data`, update its path the same way.

- [ ] **Step 3: Check the license of 2factorauth/twofactorauth**

Run: `git ls-remote https://github.com/2factorauth/twofactorauth.git HEAD && curl -fsSL https://raw.githubusercontent.com/2factorauth/twofactorauth/master/LICENSE | head -5`
Expected: a permissive license (MIT at the time of writing). If it is not MIT/CC-BY/CC0-compatible, STOP and report to the owner: the 2FA check cannot ship with that data.

- [ ] **Step 4: Write the two-factor script**

Create `scripts/update-twofactor-directory.mjs`:

```js
#!/usr/bin/env node
// Rebuilds crates/havenkeys-core/data/twofactor-sites.json from the 2FA
// Directory by 2factorauth (https://github.com/2factorauth/twofactorauth).
// Run by hand; the output is committed and reviewed like code, and nothing
// fetches it at runtime. Only sites that offer TOTP codes are kept: Vault
// health suggests adding a one-time code to logins for them.
//
// entries/<letter>/<domain>.json: { "<Name>": { "domain", "additional-domains"?,
// "tfa"?: ["totp", "sms", ...], "documentation"? } }
//
// Usage: node scripts/update-twofactor-directory.mjs

import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "https://github.com/2factorauth/twofactorauth.git";
const OUT = join(dirname(fileURLToPath(import.meta.url)), "../crates/havenkeys-core/data/twofactor-sites.json");
const MAX_NAME = 100;

function hostname(d) {
  if (typeof d !== "string" || d.length === 0 || d.length > 253 || !d.includes(".") || d.endsWith(".")) return null;
  try {
    const h = new URL(`https://${d}/`).hostname;
    return h === d ? h : null;
  } catch {
    return null;
  }
}

function httpsUrl(u) {
  if (typeof u !== "string") return null;
  try {
    const parsed = new URL(u);
    return parsed.protocol === "https:" ? parsed.href : null;
  } catch {
    return null;
  }
}

function cleanName(n) {
  const t = typeof n === "string" ? n.trim() : "";
  // eslint-disable-next-line no-control-regex
  return t.length > 0 && t.length <= MAX_NAME && !/[\u0000-\u001f\u007f]/.test(t) ? t : null;
}

const dir = mkdtempSync(join(tmpdir(), "twofactor-dir-"));
try {
  execFileSync("git", ["clone", "--depth", "1", "--quiet", REPO, dir], { stdio: "inherit" });
  const commit = execFileSync("git", ["-C", dir, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  const sites = [];
  const entries = join(dir, "entries");
  for (const letter of readdirSync(entries)) {
    for (const file of readdirSync(join(entries, letter))) {
      if (!file.endsWith(".json")) continue;
      const data = JSON.parse(readFileSync(join(entries, letter, file), "utf8"));
      for (const [rawName, e] of Object.entries(data)) {
        const name = cleanName(rawName);
        const primary = hostname(e?.domain) ?? hostname(file.slice(0, -".json".length));
        if (!name || !primary || !Array.isArray(e?.tfa) || !e.tfa.includes("totp")) continue;
        const extra = Array.isArray(e["additional-domains"]) ? e["additional-domains"].map(hostname).filter(Boolean) : [];
        const domains = [...new Set([primary, ...extra])];
        sites.push({ name, domains, help: httpsUrl(e.documentation) });
      }
    }
  }
  sites.sort((a, b) => a.name.localeCompare(b.name, "en") || a.domains[0].localeCompare(b.domains[0]));
  writeFileSync(OUT, JSON.stringify(sites, null, 2) + "\n");
  console.error(`Wrote ${sites.length} sites from 2factorauth/twofactorauth@${commit}`);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
```

- [ ] **Step 5: Generate and inspect the data**

Run: `node scripts/update-twofactor-directory.mjs && ls -l crates/havenkeys-core/data/ && head -c 400 crates/havenkeys-core/data/twofactor-sites.json`
Expected: a "Wrote N sites" line with N in the low thousands; file roughly 200–400 KB pretty-printed (the spec's 50–100 KB estimate was for compact JSON; size is acceptable — it is compiled into the binary, record the actual size in the commit message). Spot-check that `github.com` and `google.com` are present: `grep -c '"github.com"' crates/havenkeys-core/data/twofactor-sites.json` → ≥ 1.

- [ ] **Step 6: Record the notices**

In `THIRD-PARTY-NOTICES.md`, next to the Passkeys Directory entry, add an entry for "2FA Directory by 2factorauth (https://github.com/2factorauth/twofactorauth)", its license from Step 3, the commit printed in Step 5, and "Used in: crates/havenkeys-core/data/twofactor-sites.json (Vault health)". Update the Passkeys Directory entry's path to `crates/havenkeys-core/data/passkey-sites.json`.

- [ ] **Step 7: Commit**

```bash
git add -A crates/havenkeys-core/data scripts/update-passkey-directory.mjs scripts/update-twofactor-directory.mjs apps/extension THIRD-PARTY-NOTICES.md
git commit -m "feat(health): site directories live in the core; add the TOTP directory (2factorauth)"
```

---

### Task 2: Directory parsing and lookup in Rust

**Files:**
- Create: `crates/havenkeys-core/src/health/mod.rs` (only `mod directory;` for now plus the module doc)
- Create: `crates/havenkeys-core/src/health/directory.rs`
- Modify: `crates/havenkeys-core/src/lib.rs` (add `pub mod health;` alphabetically after `generator`)

**Interfaces:**
- Consumes: `crate::origin::{host_key, registrable_domain_of}` (both `pub(crate)`).
- Produces (crate-private):
  - `pub(crate) struct Directory` with `pub(crate) fn parse(json: &str, require_passwordless: bool) -> Directory`, `pub(crate) fn lookup(&self, host: &str) -> Option<&Site>`.
  - `pub(crate) struct Site { pub(crate) help: Option<String> }`
  - `pub(crate) fn passkey_sites() -> &'static Directory`, `pub(crate) fn twofactor_sites() -> &'static Directory`
  - `pub(crate) fn rule_host(url: &str) -> Option<(String /*scheme*/, String /*host*/)>`

- [ ] **Step 1: Write the failing tests**

Create `crates/havenkeys-core/src/health/directory.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
      {"name":"GitHub","domains":["github.com"],"passwordless":true,"mfa":true,"help":"https://docs.github.com/passkeys"},
      {"name":"MfaOnly","domains":["mfa-only.com"],"passwordless":false,"mfa":true,"help":null},
      {"name":"Bad host","domains":["not a host"],"passwordless":true,"mfa":false,"help":null},
      {"name":"Http help","domains":["plain.com"],"passwordless":true,"mfa":false,"help":"http://plain.com/help"},
      {"name":"Deep","domains":["accounts.deep.com"],"passwordless":true,"mfa":false,"help":null},
      42
    ]"#;

    fn dir() -> Directory {
        Directory::parse(SAMPLE, true)
    }

    #[test]
    fn exact_and_subdomain_hosts_match() {
        let d = dir();
        assert!(d.lookup("github.com").is_some());
        assert!(d.lookup("gist.github.com").is_some());
        assert!(d.lookup("a.b.github.com").is_some());
    }

    #[test]
    fn lookalike_hosts_never_match() {
        let d = dir();
        assert!(d.lookup("evilgithub.com").is_none());
        assert!(d.lookup("github.com.evil.com").is_none());
        assert!(d.lookup("github-login.example.com").is_none());
        assert!(d.lookup("com").is_none());
    }

    #[test]
    fn a_parent_of_a_listed_subdomain_does_not_match() {
        let d = dir();
        assert!(d.lookup("deep.com").is_none());
        assert!(d.lookup("accounts.deep.com").is_some());
    }

    #[test]
    fn passwordless_filter_and_bad_entries_are_dropped() {
        let d = dir();
        assert!(d.lookup("mfa-only.com").is_none());
        assert!(Directory::parse(SAMPLE, false).lookup("mfa-only.com").is_some());
        assert!(d.lookup("plain.com").unwrap().help.is_none(), "http help links are dropped");
        assert_eq!(
            d.lookup("github.com").unwrap().help.as_deref(),
            Some("https://docs.github.com/passkeys")
        );
    }

    #[test]
    fn garbage_parses_to_an_empty_directory() {
        for junk in ["", "{}", "null", "[[[", "[{\"domains\": 5}]", "\u{0}"] {
            assert!(Directory::parse(junk, false).lookup("github.com").is_none());
        }
    }

    #[test]
    fn lookup_normalizes_hosts() {
        let d = dir();
        let (_, host) = rule_host("https://GitHub.com./login").unwrap();
        assert!(d.lookup(&host).is_some());
        let (_, idn) = rule_host("https://bücher.example/").unwrap();
        assert_eq!(idn, "xn--bcher-kva.example");
    }

    #[test]
    fn rule_host_only_accepts_http_and_https() {
        assert_eq!(rule_host("http://example.com").unwrap().0, "http");
        assert!(rule_host("ftp://example.com").is_none());
        assert!(rule_host("not a url").is_none());
    }

    #[test]
    fn bundled_directories_parse() {
        assert!(passkey_sites().len() > 50);
        assert!(twofactor_sites().len() > 500);
        assert!(twofactor_sites().lookup("github.com").is_some());
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p havenkeys-core health::directory`
Expected: FAIL to compile (`Directory` not defined).

- [ ] **Step 3: Implement**

At the top of `directory.rs`:

```rust
//! The two site lists Vault health checks logins against, compiled in from
//! `crates/havenkeys-core/data/` (2factorauth data, refreshed by scripts and
//! reviewed like code; never fetched). Third-party text: every entry is
//! validated, and one that fails is dropped, never a panic.
//!
//! Matching is by DNS labels, never string suffixes: a login host matches a
//! listed domain when it is that domain or a subdomain of it, and the walk
//! stops at the host's registrable domain (Public Suffix List), so
//! `evilgithub.com` and `github.com.evil.com` never match `github.com`.

use crate::origin::{host_key, registrable_domain_of};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;
use url::Url;

const MAX_SITES: usize = 20_000;
const MAX_NAME: usize = 100;

pub(crate) struct Site {
    pub(crate) help: Option<String>,
}

pub(crate) struct Directory {
    sites: Vec<Site>,
    by_domain: HashMap<String, usize>,
}

#[derive(Deserialize)]
struct RawSite {
    name: serde_json::Value,
    domains: serde_json::Value,
    #[serde(default)]
    passwordless: Option<bool>,
    #[serde(default)]
    help: serde_json::Value,
}

/// A lowercase DNS name as the URL parser would write it, or `None`.
fn clean_host(v: &serde_json::Value) -> Option<String> {
    let s = v.as_str()?;
    if s.is_empty() || s.len() > 253 || !s.contains('.') || s.ends_with('.') {
        return None;
    }
    let url = Url::parse(&format!("https://{s}/")).ok()?;
    let host = host_key(&url)?;
    (host == s).then_some(host)
}

fn clean_help(v: &serde_json::Value) -> Option<String> {
    let url = Url::parse(v.as_str()?).ok()?;
    (url.scheme() == "https" && url.host().is_some()).then(|| url.to_string())
}

fn clean_name(v: &serde_json::Value) -> bool {
    v.as_str().is_some_and(|n| {
        let n = n.trim();
        !n.is_empty() && n.chars().count() <= MAX_NAME && !n.chars().any(char::is_control)
    })
}

impl Directory {
    /// `require_passwordless`: keep only sites where a passkey replaces the
    /// password (the passkey list); the TOTP list passes `false`.
    pub(crate) fn parse(json: &str, require_passwordless: bool) -> Self {
        let mut dir = Directory { sites: Vec::new(), by_domain: HashMap::new() };
        let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(json) else {
            return dir;
        };
        for entry in entries.into_iter().take(MAX_SITES) {
            let Ok(raw) = serde_json::from_value::<RawSite>(entry) else { continue };
            if !clean_name(&raw.name) {
                continue;
            }
            if require_passwordless && raw.passwordless != Some(true) {
                continue;
            }
            let Some(list) = raw.domains.as_array() else { continue };
            let domains: Vec<String> = list.iter().filter_map(clean_host).collect();
            if domains.is_empty() {
                continue;
            }
            let idx = dir.sites.len();
            dir.sites.push(Site { help: clean_help(&raw.help) });
            for d in domains {
                dir.by_domain.entry(d).or_insert(idx);
            }
        }
        dir
    }

    pub(crate) fn len(&self) -> usize {
        self.sites.len()
    }

    /// The site listing `host` or one of its parent domains, down to (and
    /// including) the host's registrable domain. A host without one (an IP
    /// address, `localhost`, an unknown suffix) matches nothing.
    pub(crate) fn lookup(&self, host: &str) -> Option<&Site> {
        let site = registrable_domain_of(host)?;
        let mut h = host;
        loop {
            if let Some(&i) = self.by_domain.get(h) {
                return self.sites.get(i);
            }
            if h.len() <= site.len() {
                return None;
            }
            h = h.split_once('.')?.1;
        }
    }
}

/// Scheme and normalized host of a saved website rule; only http(s).
pub(crate) fn rule_host(url: &str) -> Option<(String, String)> {
    let url = Url::parse(url).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    Some((url.scheme().to_owned(), host_key(&url)?.to_ascii_lowercase()))
}

pub(crate) fn passkey_sites() -> &'static Directory {
    static DIR: OnceLock<Directory> = OnceLock::new();
    DIR.get_or_init(|| Directory::parse(include_str!("../../data/passkey-sites.json"), true))
}

pub(crate) fn twofactor_sites() -> &'static Directory {
    static DIR: OnceLock<Directory> = OnceLock::new();
    DIR.get_or_init(|| Directory::parse(include_str!("../../data/twofactor-sites.json"), false))
}
```

Create `crates/havenkeys-core/src/health/mod.rs`:

```rust
//! Vault health (spec 2026-10-07-vault-health): which logins have a weak,
//! reused or old password, an http website, a duplicate, or a site that
//! offers passkeys or one-time codes the login does not use. Computed from
//! the unlocked vault; the report holds item IDs and check kinds only.

pub(crate) mod directory;
```

Add `pub mod health;` to `lib.rs` after `pub mod generator;`.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p havenkeys-core health::directory`
Expected: PASS (8 tests). Note `lookup_normalizes_hosts`: `Url` lowercases hosts and `host_key` strips the trailing dot.

- [ ] **Step 5: Add a fuzz-style property test**

Append to the test module (the crate already relies on plain `#[test]` loops for hostile input; no new dev-dependency):

```rust
    #[test]
    fn hostile_entries_never_panic() {
        let cases = [
            r#"[{"name":"x","domains":["a.com"],"help":{"nested":[1,2]}}]"#,
            r#"[{"name":"\u0007bell","domains":["a.com"]}]"#,
            r#"[{"name":"x","domains":[".", "..", "a..com", "-a.com", "a.com."]}]"#,
            r#"[{"name":"x","domains":["xn--"]}]"#,
        ];
        for c in cases {
            let d = Directory::parse(c, false);
            let _ = d.lookup("a.com");
        }
        let long = format!(r#"[{{"name":"x","domains":["{}.com"]}}]"#, "a".repeat(300));
        assert!(Directory::parse(&long, false).lookup("a.com").is_none());
    }
```

Run: `cargo test -p havenkeys-core health::directory` → PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-core/src/health crates/havenkeys-core/src/lib.rs
git commit -m "feat(health): parse and match the bundled passkey and TOTP site lists"
```

---

### Task 3: Report types, the `health_ignored` field and `compute`

**Files:**
- Modify: `crates/havenkeys-core/Cargo.toml` (add `zxcvbn`)
- Modify: `crates/havenkeys-core/src/health/mod.rs`
- Modify: `crates/havenkeys-core/src/model.rs` (`ItemDetails::Login` gains `health_ignored`)
- Modify: `crates/havenkeys-core/src/vault.rs` (`build_item` carries it; `dedupe_key_parts` becomes `pub(crate)`)
- Modify: every other `ItemDetails::Login { … }` constructor/destructure the compiler flags (`app_fill.rs`, `passkey/vault.rs`, `export/*.rs`, `export/restore.rs`, `model.rs` tests)

**Interfaces:**
- Consumes: Task 2's `directory::{passkey_sites, twofactor_sites, rule_host}`, `crate::origin::registrable_domain_of`.
- Produces (public, `havenkeys_core::health`):

```rust
pub enum HealthCheck { Weak, Reused, Old, Passkey, TwoFactor, Insecure, Duplicate }
pub struct HealthCounts { pub weak: u32, pub reused: u32, pub old: u32, pub passkey: u32,
                          pub two_factor: u32, pub insecure: u32, pub duplicate: u32 }
pub struct HealthIssue { pub item_id: Uuid, pub checks: Vec<HealthCheck>,
                         pub reused_group: Option<u32>, pub duplicate_group: Option<u32> }
pub struct HealthDismissed { pub item_id: Uuid, pub checks: Vec<HealthCheck> }
pub struct HealthReport { pub computed_at: i64, pub counts: HealthCounts,
                          pub issues: Vec<HealthIssue>, pub dismissed: Vec<HealthDismissed> }
pub struct HealthSnapshot { /* crate-private fields */ }
pub fn compute(snapshot: &HealthSnapshot, now_ms: i64) -> HealthReport
pub const OLD_AFTER_MS: i64 = 365 * 24 * 3600 * 1000;
```
- Crate-private: `pub(crate) struct LoginFacts { id, title, username, hosts: Vec<(String,String)>, password: Option<SecretString>, last_changed: i64, has_totp, has_passkey, ignored: Vec<HealthCheck>, dedupe: [u8; 32] }`, `HealthSnapshot { pub(crate) logins: Vec<LoginFacts>, pub(crate) generation: u64, pub(crate) epoch: u64 }`.

- [ ] **Step 1: Add the dependency and check it**

In `crates/havenkeys-core/Cargo.toml`, under `# --- encoding / parsing ---`:

```toml
# Password strength for Vault health (Dropbox's zxcvbn, Rust port by
# shssoichiro; MIT). No builder feature: we only call `zxcvbn()`.
zxcvbn = { version = "3.1", default-features = false }
```

Run: `cargo build -p havenkeys-core && cargo deny check && cargo audit`
Expected: builds; deny and audit report nothing new. If either flags a zxcvbn dependency, STOP and report it.

- [ ] **Step 2: Add the field to the model**

In `crates/havenkeys-core/src/model.rs`, inside `ItemDetails::Login` after `app_bindings`:

```rust
        /// Vault health checks the user dismissed for this login (spec
        /// 2026-10-07-vault-health §4.8). Carried over by every edit.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        health_ignored: Vec<crate::health::HealthCheck>,
```

In `vault.rs::build_item`, carry it exactly like `app_bindings`: add it to the destructured tuple from `current` (default `Vec::new()`) and to the constructed `ItemDetails::Login { …, app_bindings, health_ignored }`.

Change `fn dedupe_key_parts` in `vault.rs` to `pub(crate) fn dedupe_key_parts`.

Run: `cargo build -p havenkeys-core --all-targets 2>&1 | grep -E "^error" -A6`
For each error: a constructor of a **new** login (passkey save, app fill new login, import, restore of a backup that predates the field) uses `health_ignored: Vec::new()` — except `export/restore.rs`, which must carry the value from the backup if the backup's details are deserialized as `ItemDetails` (then nothing to do) or construct it from the backup's field; a destructure that does not need it adds `..` or `health_ignored: _`. Bitwarden/CSV exports ignore it.

- [ ] **Step 3: Write the failing tests for `compute`**

In `health/mod.rs`, add a test module (the types do not exist yet):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::SecretString;
    use uuid::Uuid;

    const NOW: i64 = 1_800_000_000_000;
    const STRONG: &str = "q7$Vt!m2Lz#9pWx4Rk@e";

    fn facts(n: u128, url: &str, password: Option<&str>) -> LoginFacts {
        LoginFacts {
            id: Uuid::from_u128(n),
            title: format!("Login {n}"),
            username: Some(format!("user{n}")),
            hosts: directory::rule_host(url).into_iter().collect(),
            password: password.map(SecretString::from),
            last_changed: NOW,
            has_totp: false,
            has_passkey: false,
            ignored: Vec::new(),
            dedupe: [n as u8; 32],
        }
    }

    fn report(logins: Vec<LoginFacts>) -> HealthReport {
        compute(&HealthSnapshot { logins, generation: 0, epoch: 0 }, NOW)
    }

    fn checks(r: &HealthReport, n: u128) -> Vec<HealthCheck> {
        r.issues.iter().find(|i| i.item_id == Uuid::from_u128(n)).map(|i| i.checks.clone()).unwrap_or_default()
    }

    #[test]
    fn weak_passwords_are_flagged_and_strong_ones_are_not() {
        let r = report(vec![
            facts(1, "https://unlisted-site.example", Some("password1")),
            facts(2, "https://unlisted-site.example", Some(STRONG)),
        ]);
        assert!(checks(&r, 1).contains(&HealthCheck::Weak));
        assert!(!checks(&r, 2).contains(&HealthCheck::Weak));
        assert_eq!(r.counts.weak, 1);
    }

    #[test]
    fn a_password_built_from_the_title_is_weak() {
        let mut f = facts(1, "https://unlisted-site.example", Some("Zephyrwind2024"));
        f.title = "Zephyrwind".into();
        assert!(checks(&report(vec![f]), 1).contains(&HealthCheck::Weak));
    }

    #[test]
    fn reused_passwords_form_groups_ordered_by_item_id() {
        let r = report(vec![
            facts(9, "https://a.example", Some(STRONG)),
            facts(3, "https://b.example", Some(STRONG)),
            facts(5, "https://c.example", Some("another-Strong-pass-77!")),
            facts(1, "https://d.example", Some("another-Strong-pass-77!")),
            facts(7, "https://e.example", Some("unique-Strong-pass-99?")),
        ]);
        let g = |n| r.issues.iter().find(|i| i.item_id == Uuid::from_u128(n)).and_then(|i| i.reused_group);
        assert_eq!(g(1), Some(0));
        assert_eq!(g(5), Some(0));
        assert_eq!(g(3), Some(1));
        assert_eq!(g(9), Some(1));
        assert_eq!(g(7), None);
        assert_eq!(r.counts.reused, 4);
    }

    #[test]
    fn empty_and_missing_passwords_are_not_reused() {
        let r = report(vec![
            facts(1, "https://a.example", Some("")),
            facts(2, "https://b.example", Some("")),
            facts(3, "https://c.example", None),
            facts(4, "https://d.example", None),
        ]);
        assert_eq!(r.counts.reused, 0);
    }

    #[test]
    fn old_is_strictly_more_than_a_year() {
        let mut exactly = facts(1, "https://a.example", Some(STRONG));
        exactly.last_changed = NOW - OLD_AFTER_MS;
        let mut older = facts(2, "https://b.example", Some("other-Strong-pass-12#"));
        older.last_changed = NOW - OLD_AFTER_MS - 1;
        let mut no_password = facts(3, "https://c.example", None);
        no_password.last_changed = 0;
        let r = report(vec![exactly, older, no_password]);
        assert!(!checks(&r, 1).contains(&HealthCheck::Old));
        assert!(checks(&r, 2).contains(&HealthCheck::Old));
        assert!(!checks(&r, 3).contains(&HealthCheck::Old));
    }

    #[test]
    fn passkey_sites_win_over_two_factor() {
        // github.com is in both bundled lists.
        let r = report(vec![facts(1, "https://github.com", Some(STRONG))]);
        assert!(checks(&r, 1).contains(&HealthCheck::Passkey));
        assert!(!checks(&r, 1).contains(&HealthCheck::TwoFactor));
    }

    #[test]
    fn a_login_with_a_passkey_gets_the_two_factor_suggestion_instead() {
        let mut f = facts(1, "https://github.com", Some(STRONG));
        f.has_passkey = true;
        let r = report(vec![f]);
        assert!(!checks(&r, 1).contains(&HealthCheck::Passkey));
        assert!(checks(&r, 1).contains(&HealthCheck::TwoFactor));
    }

    #[test]
    fn lookalike_hosts_get_no_site_suggestion() {
        for url in ["https://evilgithub.com", "https://github.com.evil.com", "https://github-login.example.com"] {
            let r = report(vec![facts(1, url, Some(STRONG))]);
            assert!(!checks(&r, 1).contains(&HealthCheck::Passkey), "{url}");
            assert!(!checks(&r, 1).contains(&HealthCheck::TwoFactor), "{url}");
        }
    }

    #[test]
    fn a_login_with_totp_is_not_asked_for_two_factor() {
        let mut f = facts(1, "https://github.com", Some(STRONG));
        f.has_passkey = true;
        f.has_totp = true;
        assert!(checks(&report(vec![f]), 1).is_empty());
    }

    #[test]
    fn http_websites_are_unsecured_except_local_devices() {
        let flagged = report(vec![facts(1, "http://example.com", Some(STRONG))]);
        assert!(checks(&flagged, 1).contains(&HealthCheck::Insecure));
        for url in ["https://example.com", "http://localhost:8080", "http://192.168.0.1", "http://router.lan"] {
            let r = report(vec![facts(1, url, Some(STRONG))]);
            assert!(!checks(&r, 1).contains(&HealthCheck::Insecure), "{url}");
        }
    }

    #[test]
    fn duplicates_group_by_digest_regardless_of_password() {
        let mut a = facts(1, "https://a.example", Some(STRONG));
        let mut b = facts(2, "https://a.example", Some("different-Strong-pass-3$"));
        a.dedupe = [7; 32];
        b.dedupe = [7; 32];
        let c = facts(3, "https://a.example", Some("third-Strong-pass-4%"));
        let r = report(vec![a, b, c]);
        assert!(checks(&r, 1).contains(&HealthCheck::Duplicate));
        assert!(checks(&r, 2).contains(&HealthCheck::Duplicate));
        assert!(!checks(&r, 3).contains(&HealthCheck::Duplicate));
        assert_eq!(r.counts.duplicate, 2);
    }

    #[test]
    fn dismissed_checks_move_out_of_issues_and_counts() {
        let mut f = facts(1, "http://example.com", Some("password1"));
        f.ignored = vec![HealthCheck::Weak];
        let r = report(vec![f]);
        assert_eq!(checks(&r, 1), vec![HealthCheck::Insecure]);
        assert_eq!(r.counts.weak, 0);
        assert_eq!(r.dismissed.len(), 1);
        assert_eq!(r.dismissed[0].checks, vec![HealthCheck::Weak]);
    }

    #[test]
    fn a_login_with_nothing_wrong_is_not_listed() {
        let r = report(vec![facts(1, "https://unlisted-site.example", Some(STRONG))]);
        assert!(r.issues.is_empty());
        assert!(r.dismissed.is_empty());
    }

    #[test]
    fn long_and_unicode_passwords_do_not_panic() {
        let long = "a".repeat(10_000);
        let r = report(vec![
            facts(1, "https://a.example", Some(&long)),
            facts(2, "https://b.example", Some("🔐🔐🔐ção-Ünïcode-ŝtrong-✓")),
        ]);
        assert_eq!(r.computed_at, NOW);
    }

    #[test]
    fn the_serialized_report_never_contains_a_password() {
        let pw = "Leaky-Secret-Pass-0042";
        let r = report(vec![
            facts(1, "https://github.com", Some(pw)),
            facts(2, "https://github.com", Some(pw)),
        ]);
        let json = serde_json::to_string(&r).unwrap();
        for window in pw.as_bytes().windows(4) {
            assert!(!json.contains(std::str::from_utf8(window).unwrap()), "leaked {:?}", window);
        }
        assert!(!format!("{r:?}").contains("Leaky"));
    }
}
```

Note on the leak test: 4-byte windows of the password like `"Pass"` could appear in a field name. The report's field names are `computedAt, counts, weak, reused, old, passkey, twoFactor, insecure, duplicate, issues, itemId, checks, reusedGroup, duplicateGroup, dismissed` and the check values. If a window collides with one of them, pick a password with no such substring rather than weakening the assertion.

- [ ] **Step 4: Run them to see them fail**

Run: `cargo test -p havenkeys-core health::tests`
Expected: FAIL to compile (`compute`, `LoginFacts` … not defined).

- [ ] **Step 5: Implement the types and `compute`**

Add to `health/mod.rs` (above the test module):

```rust
use crate::secret::SecretString;
use directory::{passkey_sites, twofactor_sites};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use uuid::Uuid;
use zeroize::Zeroize;

/// A password unchanged for longer than this is "old".
pub const OLD_AFTER_MS: i64 = 365 * 24 * 3600 * 1000;
/// zxcvbn scores below this are "weak" (0, 1, 2 of 0–4).
const STRONG_ENOUGH: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthCheck {
    Weak,
    Reused,
    Old,
    Passkey,
    TwoFactor,
    Insecure,
    Duplicate,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCounts {
    pub weak: u32,
    pub reused: u32,
    pub old: u32,
    pub passkey: u32,
    pub two_factor: u32,
    pub insecure: u32,
    pub duplicate: u32,
}

impl HealthCounts {
    fn add(&mut self, check: HealthCheck) {
        let slot = match check {
            HealthCheck::Weak => &mut self.weak,
            HealthCheck::Reused => &mut self.reused,
            HealthCheck::Old => &mut self.old,
            HealthCheck::Passkey => &mut self.passkey,
            HealthCheck::TwoFactor => &mut self.two_factor,
            HealthCheck::Insecure => &mut self.insecure,
            HealthCheck::Duplicate => &mut self.duplicate,
        };
        *slot += 1;
    }
}

/// One login's problems. IDs and kinds only (spec §5.1).
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthIssue {
    pub item_id: Uuid,
    pub checks: Vec<HealthCheck>,
    pub reused_group: Option<u32>,
    pub duplicate_group: Option<u32>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDismissed {
    pub item_id: Uuid,
    pub checks: Vec<HealthCheck>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub computed_at: i64,
    pub counts: HealthCounts,
    pub issues: Vec<HealthIssue>,
    pub dismissed: Vec<HealthDismissed>,
}

impl fmt::Debug for HealthReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HealthReport")
            .field("counts", &self.counts)
            .field("issues", &self.issues.len())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for HealthIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HealthIssue").field("item_id", &self.item_id).finish_non_exhaustive()
    }
}

impl fmt::Debug for HealthDismissed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HealthDismissed").field("item_id", &self.item_id).finish_non_exhaustive()
    }
}

/// What `compute` needs to know about one login, read under the vault
/// lock. Holds the password; dropped (zeroized) as soon as the report is built.
pub(crate) struct LoginFacts {
    pub(crate) id: Uuid,
    pub(crate) title: String,
    pub(crate) username: Option<String>,
    /// (scheme, host) of each http(s) website rule.
    pub(crate) hosts: Vec<(String, String)>,
    pub(crate) password: Option<SecretString>,
    pub(crate) last_changed: i64,
    pub(crate) has_totp: bool,
    pub(crate) has_passkey: bool,
    pub(crate) ignored: Vec<HealthCheck>,
    /// `vault::dedupe_key_parts` digest (no password in it).
    pub(crate) dedupe: [u8; 32],
}

impl Drop for LoginFacts {
    fn drop(&mut self) {
        self.title.zeroize();
        self.username.zeroize();
        for (_, h) in &mut self.hosts {
            h.zeroize();
        }
    }
}

/// The logins as they were when the snapshot was taken, and which session
/// state (`generation`, `epoch`) that was, so a stale report is not cached.
pub struct HealthSnapshot {
    pub(crate) logins: Vec<LoginFacts>,
    pub(crate) generation: u64,
    pub(crate) epoch: u64,
}

impl fmt::Debug for HealthSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HealthSnapshot").field("logins", &self.logins.len()).finish_non_exhaustive()
    }
}

fn is_weak(f: &LoginFacts, password: &str) -> bool {
    let mut inputs: Vec<&str> = vec![f.title.as_str()];
    if let Some(u) = f.username.as_deref() {
        inputs.push(u);
    }
    u8::from(zxcvbn::zxcvbn(password, &inputs).score()) < STRONG_ENOUGH
}

/// Groups of two or more logins sharing a key, numbered by each group's
/// smallest item ID. Returns login index → group number.
fn groups<K: std::hash::Hash + Eq>(keys: impl Iterator<Item = (usize, K)>, ids: &[Uuid]) -> HashMap<usize, u32> {
    let mut by_key: HashMap<K, Vec<usize>> = HashMap::new();
    for (i, k) in keys {
        by_key.entry(k).or_default().push(i);
    }
    let mut multi: Vec<Vec<usize>> = by_key.into_values().filter(|v| v.len() > 1).collect();
    multi.sort_by_key(|v| v.iter().map(|&i| ids[i]).min());
    let mut out = HashMap::new();
    for (g, members) in multi.into_iter().enumerate() {
        for i in members {
            out.insert(i, g as u32);
        }
    }
    out
}

pub fn compute(snapshot: &HealthSnapshot, now_ms: i64) -> HealthReport {
    let logins = &snapshot.logins;
    let ids: Vec<Uuid> = logins.iter().map(|f| f.id).collect();
    let reused = groups(
        logins.iter().enumerate().filter_map(|(i, f)| {
            let p = f.password.as_ref()?.expose();
            (!p.is_empty()).then_some((i, p))
        }),
        &ids,
    );
    let duplicate = groups(logins.iter().enumerate().map(|(i, f)| (i, f.dedupe)), &ids);

    let mut counts = HealthCounts::default();
    let mut issues = Vec::new();
    let mut dismissed = Vec::new();
    // BTreeMap: one deterministic order (by item ID) for the UI.
    let mut ordered: BTreeMap<Uuid, usize> = BTreeMap::new();
    for (i, f) in logins.iter().enumerate() {
        ordered.insert(f.id, i);
    }
    for (_, i) in ordered {
        let f = &logins[i];
        let mut found = Vec::new();
        let password = f.password.as_ref().map(|p| p.expose()).filter(|p| !p.is_empty());
        if let Some(p) = password {
            if is_weak(f, p) {
                found.push(HealthCheck::Weak);
            }
            if reused.contains_key(&i) {
                found.push(HealthCheck::Reused);
            }
            if now_ms - f.last_changed > OLD_AFTER_MS {
                found.push(HealthCheck::Old);
            }
        }
        let hosts = || f.hosts.iter().map(|(_, h)| h.as_str());
        let passkey = !f.has_passkey && hosts().any(|h| passkey_sites().lookup(h).is_some());
        if passkey {
            found.push(HealthCheck::Passkey);
        } else if !f.has_totp && hosts().any(|h| twofactor_sites().lookup(h).is_some()) {
            found.push(HealthCheck::TwoFactor);
        }
        if f.hosts.iter().any(|(s, h)| s == "http" && crate::origin::registrable_domain_of(h).is_some()) {
            found.push(HealthCheck::Insecure);
        }
        if duplicate.contains_key(&i) {
            found.push(HealthCheck::Duplicate);
        }
        let (gone, kept): (Vec<_>, Vec<_>) = found.into_iter().partition(|c| f.ignored.contains(c));
        for c in &kept {
            counts.add(*c);
        }
        if !gone.is_empty() {
            dismissed.push(HealthDismissed { item_id: f.id, checks: gone });
        }
        if !kept.is_empty() {
            let has = |c| kept.contains(&c);
            issues.push(HealthIssue {
                item_id: f.id,
                reused_group: has(HealthCheck::Reused).then(|| reused[&i]),
                duplicate_group: has(HealthCheck::Duplicate).then(|| duplicate[&i]),
                checks: kept,
            });
        }
    }
    HealthReport { computed_at: now_ms, counts, issues, dismissed }
}
```

Note for the reviewer: `groups` keys the reused map with `&str` borrowed from the snapshot's `SecretString`s; the map is dropped when `compute` returns and holds no copy. zxcvbn copies the first 100 characters internally and keeps matched substrings in `Entropy`, which it drops without zeroizing; this is recorded as a known limitation in Task 10.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p havenkeys-core health`
Expected: PASS. If `a_password_built_from_the_title_is_weak` fails because zxcvbn scores the password 3, keep the test's intent and lower its randomness (e.g. `"Zephyrwind1"`) — the point is that the title counts as a user input.

- [ ] **Step 7: Run the whole crate**

Run: `cargo test -p havenkeys-core && cargo clippy -p havenkeys-core --all-targets -- -D warnings`
Expected: PASS. (The "every edit path keeps the field" test needs `stage_health_ignored` and lives in Task 4.)

- [ ] **Step 8: Commit**

```bash
git add crates/havenkeys-core Cargo.lock
git commit -m "feat(health): compute weak, reused, old, unsecured, duplicate, passkey and 2FA checks"
```

---

### Task 4: `VaultService` integration (snapshot, cache, dismiss, help link)

**Files:**
- Create: `crates/havenkeys-core/src/health/vault.rs`
- Modify: `crates/havenkeys-core/src/health/mod.rs` (`mod vault;`)
- Modify: `crates/havenkeys-core/src/vault.rs` (`Session` gains `health` + `generation`; `open_session` initializes them; `commit_write` bumps)
- Modify: `crates/havenkeys-core/src/sync.rs:455-460` (bump after applying a pull)

**Interfaces:**
- Consumes: Task 3's `HealthSnapshot`, `LoginFacts`, `HealthReport`, `HealthCheck`, `compute`; `vault::dedupe_key_parts`; `VaultService::{session, session_mut, load_details, get_item, stage}`; `directory::{passkey_sites, twofactor_sites, rule_host}`.
- Produces (public on `VaultService`):

```rust
pub fn cached_health(&self, now_ms: i64) -> Result<Option<HealthReport>>;
pub fn health_snapshot(&self) -> Result<HealthSnapshot>;
pub fn store_health(&mut self, snapshot: &HealthSnapshot, report: &HealthReport) -> bool; // true if cached
pub fn health_report(&mut self, now_ms: i64) -> Result<HealthReport>; // all three, under the lock (tests, small callers)
pub fn stage_health_ignored(&self, id: &Uuid, checks: Vec<HealthCheck>) -> Result<StagedWrite>;
pub fn health_help_url(&self, id: &Uuid, check: HealthCheck) -> Result<String>;
pub const HEALTH_CACHE_MS: i64 = 10 * 60_000; // in health::vault, re-exported from health
```

- [ ] **Step 1: Write the failing tests**

Create `crates/havenkeys-core/src/health/vault.rs` with this test module (implementation added in Step 3):

```rust
#[cfg(test)]
mod tests {
    use crate::health::{HealthCheck, HEALTH_CACHE_MS};
    use crate::local::tests::unlocked_vault;
    use crate::model::{ItemInput, ItemType, MatchType, SecretUpdate, UrlRule};
    use crate::vault::VaultService;
    use crate::{Error, SecretString};
    use uuid::Uuid;

    const NOW: i64 = 1_800_000_000_000;

    fn add(v: &mut VaultService, title: &str, url: &str, pw: &str) -> Uuid {
        let input = ItemInput {
            username: Some("octo".into()),
            urls: vec![UrlRule { url: url.into(), match_type: MatchType::Domain }],
            password: SecretUpdate::Set(SecretString::from(pw)),
            ..ItemInput::blank(ItemType::Login, title.into())
        };
        let staged = v.stage_create(input, NOW).unwrap();
        let id = staged.item_id;
        v.commit_write(staged, 1).unwrap();
        id
    }

    fn dismiss(v: &mut VaultService, id: &Uuid, checks: Vec<HealthCheck>) {
        let staged = v.stage_health_ignored(id, checks).unwrap();
        v.commit_write(staged, 2).unwrap();
    }

    #[test]
    fn the_report_lists_logins_only() {
        let mut v = unlocked_vault();
        let id = add(&mut v, "Weak", "https://unlisted-site.example", "password1");
        let note = ItemInput {
            content: SecretUpdate::Set(SecretString::from("password1")),
            ..ItemInput::blank(ItemType::SecureNote, "Note".into())
        };
        let staged = v.stage_create(note, NOW).unwrap();
        v.commit_write(staged, 1).unwrap();
        let r = v.health_report(NOW).unwrap();
        assert_eq!(r.issues.len(), 1);
        assert_eq!(r.issues[0].item_id, id);
    }

    #[test]
    fn old_comes_from_the_history_or_the_creation_date() {
        let mut v = unlocked_vault();
        let id = add(&mut v, "Site", "https://unlisted-site.example", "q7$Vt!m2Lz#9pWx4Rk@e");
        let later = NOW + crate::health::OLD_AFTER_MS + 1;
        assert!(v.health_report(later).unwrap().issues[0].checks.contains(&HealthCheck::Old));
        // Change the password a year later: no longer old.
        let edit = ItemInput {
            username: Some("octo".into()),
            urls: vec![UrlRule { url: "https://unlisted-site.example".into(), match_type: MatchType::Domain }],
            password: SecretUpdate::Set(SecretString::from("Another-q7$Vt!m2Lz#9")),
            ..ItemInput::blank(ItemType::Login, "Site".into())
        };
        let staged = v.stage_update(&id, edit, later).unwrap();
        v.commit_write(staged, 3).unwrap();
        assert!(v.health_report(later).unwrap().issues.is_empty());
    }

    #[test]
    fn dismissing_moves_the_check_and_keeps_updated_at() {
        let mut v = unlocked_vault();
        let id = add(&mut v, "Weak", "https://unlisted-site.example", "password1");
        let before = v.get_item(&id).unwrap().updated_at;
        dismiss(&mut v, &id, vec![HealthCheck::Weak, HealthCheck::Weak]);
        assert_eq!(v.get_item(&id).unwrap().updated_at, before);
        let r = v.health_report(NOW).unwrap();
        assert!(r.issues.is_empty());
        assert_eq!(r.dismissed[0].checks, vec![HealthCheck::Weak]);
        // Undo.
        dismiss(&mut v, &id, vec![]);
        assert_eq!(v.health_report(NOW).unwrap().counts.weak, 1);
    }

    #[test]
    fn every_edit_path_keeps_health_ignored() {
        let mut v = unlocked_vault();
        let id = add(&mut v, "Weak", "https://unlisted-site.example", "password1");
        dismiss(&mut v, &id, vec![HealthCheck::Weak]);
        // A full editor save (desktop and Android both go through stage_update).
        let edit = ItemInput {
            username: Some("renamed".into()),
            urls: vec![UrlRule { url: "https://unlisted-site.example".into(), match_type: MatchType::Domain }],
            ..ItemInput::blank(ItemType::Login, "Weak (renamed)".into())
        };
        let staged = v.stage_update(&id, edit, NOW + 1).unwrap();
        v.commit_write(staged, 3).unwrap();
        assert_eq!(v.health_report(NOW).unwrap().counts.weak, 0, "kept after an editor save");
        // A password changed from the browser (save prompt → "Update").
        let save = v
            .stage_save_login(
                "https://unlisted-site.example/login",
                None,
                Some("renamed"),
                SecretString::from("password2"),
                crate::vault::SaveTarget::Update(&id),
                NOW + 2,
            )
            .unwrap();
        v.commit_write(save.write, 4).unwrap();
        let r = v.health_report(NOW + 2).unwrap();
        assert_eq!(r.dismissed.len(), 1, "kept after a browser save");
        assert_eq!(r.dismissed[0].checks, vec![HealthCheck::Weak]);
    }

    #[test]
    fn only_logins_can_be_dismissed() {
        let mut v = unlocked_vault();
        let note = ItemInput {
            content: SecretUpdate::Set(SecretString::from("x")),
            ..ItemInput::blank(ItemType::SecureNote, "Note".into())
        };
        let staged = v.stage_create(note, NOW).unwrap();
        let id = staged.item_id;
        v.commit_write(staged, 1).unwrap();
        assert_eq!(v.stage_health_ignored(&id, vec![HealthCheck::Weak]).err(), Some(Error::Denied));
        assert_eq!(v.stage_health_ignored(&Uuid::from_u128(1), vec![]).err(), Some(Error::NotFound));
    }

    #[test]
    fn a_locked_vault_answers_locked() {
        let mut v = unlocked_vault();
        let id = add(&mut v, "Weak", "https://unlisted-site.example", "password1");
        v.lock();
        assert_eq!(v.health_report(NOW).err(), Some(Error::Locked));
        assert_eq!(v.health_snapshot().err(), Some(Error::Locked));
        assert_eq!(v.cached_health(NOW).err(), Some(Error::Locked));
        assert_eq!(v.stage_health_ignored(&id, vec![]).err(), Some(Error::Locked));
        assert_eq!(v.health_help_url(&id, HealthCheck::Passkey).err(), Some(Error::Locked));
    }

    #[test]
    fn the_cache_is_cleared_by_writes_and_expires() {
        let mut v = unlocked_vault();
        add(&mut v, "Weak", "https://unlisted-site.example", "password1");
        v.health_report(NOW).unwrap();
        assert!(v.cached_health(NOW + 1).unwrap().is_some());
        assert!(v.cached_health(NOW + HEALTH_CACHE_MS + 1).unwrap().is_none());
        add(&mut v, "Weak 2", "https://other.example", "password1");
        assert!(v.cached_health(NOW + 1).unwrap().is_none());
    }

    #[test]
    fn a_report_from_before_a_change_or_a_lock_is_not_cached() {
        let mut v = unlocked_vault();
        add(&mut v, "Weak", "https://unlisted-site.example", "password1");
        let snap = v.health_snapshot().unwrap();
        let report = crate::health::compute(&snap, NOW);
        add(&mut v, "Weak 2", "https://other.example", "password1");
        assert!(!v.store_health(&snap, &report));
        assert!(v.cached_health(NOW).unwrap().is_none());

        let snap = v.health_snapshot().unwrap();
        let report = crate::health::compute(&snap, NOW);
        v.lock();
        assert!(!v.store_health(&snap, &report));
    }

    #[test]
    fn help_links_come_from_the_directory_for_the_login_s_own_site() {
        let mut v = unlocked_vault();
        let gh = add(&mut v, "GitHub", "https://github.com", "q7$Vt!m2Lz#9pWx4Rk@e");
        let other = add(&mut v, "Other", "https://unlisted-site.example", "q7$Vt!m2Lz#9pWx4Rk@e");
        let url = v.health_help_url(&gh, HealthCheck::Passkey).unwrap();
        assert!(url.starts_with("https://"));
        assert_eq!(v.health_help_url(&other, HealthCheck::Passkey).err(), Some(Error::NotFound));
        assert_eq!(
            v.health_help_url(&gh, HealthCheck::Weak).err(),
            Some(Error::InvalidInput("no help link for this check"))
        );
    }
}
```

`SaveTarget::Update` takes `&Uuid` (check `pub enum SaveTarget<'a>` in `vault.rs`); the passkey and Android-binding save paths build their details from `load_details` and edit one field in place, so they keep the field by construction — Task 3 Step 2's compiler pass is what proves it.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p havenkeys-core health::vault`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

In `vault.rs`, add to `Session`:

```rust
    /// Bumped on every change to the items (a write, a sync pull), so a
    /// health report computed from an older snapshot is not cached.
    pub(crate) generation: u64,
    /// The last health report and when it was computed; dropped on lock.
    pub(crate) health: Option<crate::health::HealthReport>,
```

initialize both in `open_session` (`generation: 0, health: None`), and in `commit_write`, right after `self.session()?;` succeeds and before the `match`, add:

```rust
        {
            let s = self.session_mut()?;
            s.generation = s.generation.wrapping_add(1);
            s.health = None;
        }
```

In `sync.rs`, after the two loops that insert/remove overviews (line ~460), add the same block.

Create the implementation at the top of `health/vault.rs`:

```rust
//! Vault health on the unlocked vault: snapshot, cache, dismissals, help links.

use super::directory::{passkey_sites, rule_host, twofactor_sites};
use super::{HealthCheck, HealthReport, HealthSnapshot, LoginFacts};
use crate::error::{Error, Result};
use crate::model::{ItemDetails, ItemType};
use crate::vault::{dedupe_key_parts, StagedWrite, VaultService};
use uuid::Uuid;

/// A cached report older than this is computed again (spec §5.3).
pub const HEALTH_CACHE_MS: i64 = 10 * 60_000;

impl VaultService {
    pub fn cached_health(&self, now_ms: i64) -> Result<Option<HealthReport>> {
        let s = self.session()?;
        Ok(s.health
            .as_ref()
            .filter(|r| (0..=HEALTH_CACHE_MS).contains(&(now_ms - r.computed_at)))
            .cloned())
    }

    /// Reads every login's password and facts. Call under the vault lock,
    /// then release the lock and run `health::compute` on the result.
    pub fn health_snapshot(&self) -> Result<HealthSnapshot> {
        let s = self.session()?;
        let mut logins = Vec::new();
        for ov in s.overviews.values().filter(|o| o.item_type == ItemType::Login) {
            // An item whose details do not open is skipped, as search skips it.
            let Ok(ItemDetails::Login { password, password_history, health_ignored, .. }) =
                self.load_details(&ov.id)
            else {
                continue;
            };
            logins.push(LoginFacts {
                id: ov.id,
                title: ov.title.clone(),
                username: ov.username.clone(),
                hosts: ov.urls.iter().filter_map(|r| rule_host(&r.url)).collect(),
                password,
                last_changed: password_history.first().map_or(ov.created_at, |p| p.replaced_at),
                has_totp: ov.has_totp,
                has_passkey: ov.has_passkey,
                ignored: health_ignored,
                dedupe: dedupe_key_parts(ov, None),
            });
        }
        Ok(HealthSnapshot { logins, generation: s.generation, epoch: self.epoch() })
    }

    /// Cache `report` if nothing changed since `snapshot` was taken.
    pub fn store_health(&mut self, snapshot: &HealthSnapshot, report: &HealthReport) -> bool {
        if snapshot.epoch != self.epoch() {
            return false;
        }
        match self.session_mut() {
            Ok(s) if s.generation == snapshot.generation => {
                s.health = Some(report.clone());
                true
            }
            _ => false,
        }
    }

    /// Everything under the lock. For tests and callers without a worker
    /// thread; the apps use `HavenClient::health_report`.
    pub fn health_report(&mut self, now_ms: i64) -> Result<HealthReport> {
        if let Some(r) = self.cached_health(now_ms)? {
            return Ok(r);
        }
        let snapshot = self.health_snapshot()?;
        let report = super::compute(&snapshot, now_ms);
        self.store_health(&snapshot, &report);
        Ok(report)
    }

    /// Replace the login's dismissed checks. Keeps `updated_at`: dismissing
    /// is not an edit the user would look for under "recently edited".
    pub fn stage_health_ignored(&self, id: &Uuid, mut checks: Vec<HealthCheck>) -> Result<StagedWrite> {
        let overview = self.get_item(id)?;
        if overview.item_type != ItemType::Login {
            return Err(Error::Denied);
        }
        checks.sort_unstable();
        checks.dedup();
        let mut details = self.load_details(id)?;
        let ItemDetails::Login { health_ignored, .. } = &mut details else {
            return Err(Error::Corrupted);
        };
        *health_ignored = checks;
        let base = self.store.item_revision(id)?;
        self.stage(overview, Some(&details), base)
    }

    /// The directory's https help page for the login's site.
    pub fn health_help_url(&self, id: &Uuid, check: HealthCheck) -> Result<String> {
        let dir = match check {
            HealthCheck::Passkey => passkey_sites(),
            HealthCheck::TwoFactor => twofactor_sites(),
            _ => return Err(Error::InvalidInput("no help link for this check")),
        };
        let overview = self.get_item(id)?;
        if overview.item_type != ItemType::Login {
            return Err(Error::Denied);
        }
        overview
            .urls
            .iter()
            .filter_map(|r| rule_host(&r.url))
            .find_map(|(_, host)| dir.lookup(&host).and_then(|s| s.help.clone()))
            .ok_or(Error::NotFound)
    }
}
```

`self.store` is `pub(crate)` on `VaultService`, so `self.store.item_revision` works from this module. In `health/mod.rs` add `mod vault;` and `pub use vault::HEALTH_CACHE_MS;`. Check that `Error::InvalidInput` takes a `&'static str` (it does in `vault.rs`).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p havenkeys-core && cargo clippy -p havenkeys-core --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-core
git commit -m "feat(health): snapshot, cache and dismiss on the unlocked vault"
```

---

### Task 5: `HavenClient` — compute off the lock, push dismissals

**Files:**
- Create: `crates/havenkeys-client/src/health.rs`
- Modify: `crates/havenkeys-client/src/lib.rs` (`mod health;`)

**Interfaces:**
- Consumes: `VaultService::{cached_health, health_snapshot, store_health, stage_health_ignored}`, `health::compute`, `HavenClient::{vault, require_online, push}`.
- Produces:

```rust
impl HavenClient {
    pub fn health_report(&self, now_ms: i64) -> ClientResult<HealthReport>;
    pub async fn set_health_ignored(&self, id: Uuid, checks: Vec<HealthCheck>) -> ClientResult<()>;
}
```

- [ ] **Step 1: Write the failing tests**

Look at an existing test in `crates/havenkeys-client/src/` that builds an unlocked client against the stub server (search `stub_server` / `testing.rs` for the helper, e.g. `testing::unlocked_client()`), and add in `health.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    // Use the crate's own test helper that returns an unlocked HavenClient
    // with a stub server; see testing.rs. Named `unlocked_client` below.
    use crate::testing::unlocked_client;
    use havenkeys_core::health::HealthCheck;

    #[tokio::test]
    async fn the_report_and_a_dismissal_round_trip() {
        let (client, _server) = unlocked_client().await;
        let id = crate::testing::add_login(&client, "Weak", "https://unlisted-site.example", "password1").await;
        let r = client.health_report(crate::now_ms()).unwrap();
        assert_eq!(r.counts.weak, 1);
        client.set_health_ignored(id, vec![HealthCheck::Weak]).await.unwrap();
        let r = client.health_report(crate::now_ms()).unwrap();
        assert_eq!(r.counts.weak, 0);
        assert_eq!(r.dismissed.len(), 1);
    }

    #[test]
    fn fill_is_not_blocked_while_health_computes() {
        // The vault mutex must be free while zxcvbn runs: take the snapshot,
        // then prove the lock can be taken before compute is called.
        let client = crate::testing::unlocked_client_blocking();
        let snapshot = client.vault().unwrap().health_snapshot().unwrap();
        assert!(client.vault().is_ok(), "the lock is released after the snapshot");
        let _ = havenkeys_core::health::compute(&snapshot, crate::now_ms());
    }
}
```

If the helpers have other names (`add_login`, `unlocked_client_blocking`), use the ones `testing.rs` provides; if there is no login helper, stage a create with `ItemInput` and `client.push` as `crates/havenkeys-client/src/sync.rs` tests do.

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p havenkeys-client health`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

```rust
//! Vault health for the apps: the report is computed outside the vault
//! lock (zxcvbn takes milliseconds per password), so fills never wait on it.

use crate::{ClientResult, HavenClient};
use havenkeys_core::health::{self, HealthCheck, HealthReport};
use uuid::Uuid;

impl HavenClient {
    /// Blocking: call from a worker thread (Tauri `spawn_blocking`, Android IO).
    pub fn health_report(&self, now_ms: i64) -> ClientResult<HealthReport> {
        let snapshot = {
            let vault = self.vault()?;
            if let Some(cached) = vault.cached_health(now_ms)? {
                return Ok(cached);
            }
            vault.health_snapshot()?
        };
        // The vault lock is free here: fills and other commands go ahead.
        let report = health::compute(&snapshot, now_ms);
        self.vault()?.store_health(&snapshot, &report);
        Ok(report)
    }

    pub async fn set_health_ignored(&self, id: Uuid, checks: Vec<HealthCheck>) -> ClientResult<()> {
        self.require_online()?;
        let staged = self.vault()?.stage_health_ignored(&id, checks)?;
        self.push(staged).await?;
        Ok(())
    }
}
```

The snapshot is dropped at the end of the function, after `store_health`; its passwords are zeroized by `SecretString`'s drop. `?` on `havenkeys_core::Error` converts into `ClientError` as elsewhere in this crate.

Add `mod health;` to `crates/havenkeys-client/src/lib.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p havenkeys-client && cargo clippy -p havenkeys-client --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/havenkeys-client
git commit -m "feat(health): client computes the report outside the vault lock"
```

---

### Task 6: Desktop commands

**Files:**
- Create: `apps/desktop/src-tauri/src/health.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (`mod health;` + three handlers in `generate_handler!`)
- Modify: `apps/desktop/src-tauri/build.rs` (`COMMANDS`)
- Modify: `apps/desktop/src-tauri/capabilities/main.json`
- Modify: `apps/desktop/src/lib/types.ts`, `apps/desktop/src/lib/api.ts`

**Interfaces:**
- Consumes: Task 5's `HavenClient::{health_report, set_health_ignored}`, `VaultService::health_help_url`.
- Produces: Tauri commands `health_report() -> HealthReport`, `set_health_ignored(id, checks) -> ()`, `open_health_help(id, check) -> ()`; TS `api.healthReport()`, `api.setHealthIgnored(id, checks)`, `api.openHealthHelp(id, check)`; TS types `HealthCheck`, `HealthReport`, `HealthIssue`, `HealthDismissed`, `HealthCounts`.

- [ ] **Step 1: Write the commands**

`apps/desktop/src-tauri/src/health.rs`:

```rust
//! Vault health commands. The report holds item IDs and check kinds only;
//! help links are looked up in Rust and opened here, never passed in.

use crate::state::{AppState, CmdError, CmdResult};
use havenkeys_core::health::{HealthCheck, HealthReport};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

#[tauri::command]
pub async fn health_report(app: AppHandle) -> CmdResult<HealthReport> {
    let state = app.state::<AppState>();
    state.touch();
    let client = state.client().clone();
    tauri::async_runtime::spawn_blocking(move || client.health_report(AppState::now_ms()))
        .await
        .map_err(|_| CmdError::internal())?
}

#[tauri::command]
pub async fn set_health_ignored(app: AppHandle, id: Uuid, checks: Vec<HealthCheck>) -> CmdResult<()> {
    let state = app.state::<AppState>();
    state.touch();
    let client = state.client().clone();
    client.set_health_ignored(id, checks).await
}

#[tauri::command]
pub fn open_health_help(state: State<'_, AppState>, id: Uuid, check: HealthCheck) -> CmdResult<()> {
    state.touch();
    let target = state.vault()?.health_help_url(&id, check)?;
    tauri_plugin_opener::open_url(target, None::<&str>).map_err(|_| CmdError::open_website())
}
```

Register `health::health_report, health::set_health_ignored, health::open_health_help` in `lib.rs`'s `generate_handler!`, add `"health_report", "set_health_ignored", "open_health_help"` to `COMMANDS` in `build.rs`, and `"allow-health-report", "allow-set-health-ignored", "allow-open-health-help"` to `capabilities/main.json` (keep each list's existing order/grouping style).

- [ ] **Step 2: Add the TS types and API**

`apps/desktop/src/lib/types.ts`:

```ts
export type HealthCheck = "weak" | "reused" | "old" | "passkey" | "two_factor" | "insecure" | "duplicate";

export interface HealthCounts {
  weak: number;
  reused: number;
  old: number;
  passkey: number;
  twoFactor: number;
  insecure: number;
  duplicate: number;
}

/** IDs and check kinds only: titles and usernames come from the item overviews. */
export interface HealthIssue {
  itemId: string;
  checks: HealthCheck[];
  reusedGroup: number | null;
  duplicateGroup: number | null;
}

export interface HealthDismissed {
  itemId: string;
  checks: HealthCheck[];
}

export interface HealthReport {
  computedAt: number;
  counts: HealthCounts;
  issues: HealthIssue[];
  dismissed: HealthDismissed[];
}
```

`apps/desktop/src/lib/api.ts` (next to `openWebsite`; import the new types):

```ts
  healthReport: () => call<HealthReport>("health_report"),
  setHealthIgnored: (id: string, checks: HealthCheck[]) => call<void>("set_health_ignored", { id, checks }),
  openHealthHelp: (id: string, check: HealthCheck) => call<void>("open_health_help", { id, check }),
```

- [ ] **Step 3: Run the command-surface test and Rust check**

Run: `pnpm --filter @havenkeys/desktop test -- commands && pnpm --filter @havenkeys/desktop typecheck && cargo clippy -p havenkeys-desktop -- -D warnings`
Expected: PASS (`commands.test.ts` checks build.rs, lib.rs, main.json and api.ts agree). If WebKit headers are missing, use the pkg-config stub described in `docs/development.md` ("Type-checking the Tauri crate without WebKit headers") and run `cargo check -p havenkeys-desktop`.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri apps/desktop/src/lib/types.ts apps/desktop/src/lib/api.ts
git commit -m "feat(desktop): vault health commands"
```

---

### Task 7: Desktop UI — Vault health view, sidebar badge, item chips

**Files:**
- Create: `apps/desktop/src/lib/health.ts`, `apps/desktop/src/lib/health.test.ts`
- Create: `apps/desktop/src/components/HealthChips.tsx`
- Create: `apps/desktop/src/views/HealthView.tsx`, `apps/desktop/src/views/HealthView.test.tsx`
- Modify: `apps/desktop/src/views/VaultScreen.tsx`, `apps/desktop/src/views/ItemDetail.tsx`
- Modify: `apps/desktop/src/i18n/en.ts`, `apps/desktop/src/i18n/pt-BR.ts`, `apps/desktop/src/styles.css`, `apps/desktop/src/components/Icon.tsx` (only if a needed icon is missing)

**Interfaces:**
- Consumes: Task 6's `api.healthReport/setHealthIgnored/openHealthHelp` and types; existing `ItemOverview`, `useI18n`, `useToast`, `Icon`.
- Produces:
  - `lib/health.ts`: `export const CHECK_ORDER: HealthCheck[] = ["reused", "weak", "insecure", "duplicate", "passkey", "two_factor", "old"];` `export function totalIssues(r: HealthReport): number;` `export function checksFor(r: HealthReport | null, id: string): HealthCheck[];` `export function groupSize(r: HealthReport, kind: "reused" | "duplicate", group: number): number;` `export function rowsFor(r: HealthReport, filter: HealthCheck | "dismissed" | "all"): Array<{ itemId: string; checks: HealthCheck[]; reusedGroup: number | null; duplicateGroup: number | null; dismissed: boolean }>`
  - `<HealthView items={ItemOverview[]} report={HealthReport | null} loading={boolean} readOnly={boolean} onOpen={(id) => void} onEdit={(id) => void} onChanged={() => void} />`
  - `<HealthChips checks={HealthCheck[]} report={HealthReport} itemId={string} />`
  - `Section` gains `"health"`.

- [ ] **Step 1: Write the failing helper tests**

`apps/desktop/src/lib/health.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { HealthReport } from "./types";
import { checksFor, groupSize, rowsFor, totalIssues } from "./health";

const report: HealthReport = {
  computedAt: 1,
  counts: { weak: 1, reused: 2, old: 0, passkey: 1, twoFactor: 0, insecure: 0, duplicate: 0 },
  issues: [
    { itemId: "a", checks: ["weak", "reused"], reusedGroup: 0, duplicateGroup: null },
    { itemId: "b", checks: ["reused", "passkey"], reusedGroup: 0, duplicateGroup: null },
  ],
  dismissed: [{ itemId: "c", checks: ["old"] }],
};

describe("health helpers", () => {
  it("counts every non-dismissed check", () => expect(totalIssues(report)).toBe(4));
  it("finds an item's checks", () => {
    expect(checksFor(report, "b")).toEqual(["reused", "passkey"]);
    expect(checksFor(report, "zzz")).toEqual([]);
    expect(checksFor(null, "a")).toEqual([]);
  });
  it("sizes a reuse group", () => expect(groupSize(report, "reused", 0)).toBe(2));
  it("filters rows", () => {
    expect(rowsFor(report, "weak").map((r) => r.itemId)).toEqual(["a"]);
    expect(rowsFor(report, "all").map((r) => r.itemId)).toEqual(["a", "b"]);
    expect(rowsFor(report, "dismissed")).toEqual([
      { itemId: "c", checks: ["old"], reusedGroup: null, duplicateGroup: null, dismissed: true },
    ]);
  });
});
```

Run: `pnpm --filter @havenkeys/desktop test -- health` → FAIL (module missing).

- [ ] **Step 2: Implement `lib/health.ts`**

```ts
// Vault health report helpers. The report holds IDs and check kinds only;
// titles and usernames come from the item overviews the vault already shows.

import type { HealthCheck, HealthCounts, HealthReport } from "./types";

/** Watchtower's order, then "old". */
export const CHECK_ORDER: HealthCheck[] = ["reused", "weak", "insecure", "duplicate", "passkey", "two_factor", "old"];

const COUNT_KEY: Record<HealthCheck, keyof HealthCounts> = {
  weak: "weak",
  reused: "reused",
  old: "old",
  passkey: "passkey",
  two_factor: "twoFactor",
  insecure: "insecure",
  duplicate: "duplicate",
};

export function countOf(r: HealthReport, check: HealthCheck): number {
  return r.counts[COUNT_KEY[check]];
}

export function totalIssues(r: HealthReport): number {
  return CHECK_ORDER.reduce((sum, c) => sum + countOf(r, c), 0);
}

export function checksFor(r: HealthReport | null, id: string): HealthCheck[] {
  return r?.issues.find((i) => i.itemId === id)?.checks ?? [];
}

export function groupSize(r: HealthReport, kind: "reused" | "duplicate", group: number): number {
  return r.issues.filter((i) => (kind === "reused" ? i.reusedGroup : i.duplicateGroup) === group).length;
}

export type HealthFilter = HealthCheck | "dismissed" | "all";

export interface HealthRow {
  itemId: string;
  checks: HealthCheck[];
  reusedGroup: number | null;
  duplicateGroup: number | null;
  dismissed: boolean;
}

export function rowsFor(r: HealthReport, filter: HealthFilter): HealthRow[] {
  if (filter === "dismissed") {
    return r.dismissed.map((d) => ({ itemId: d.itemId, checks: d.checks, reusedGroup: null, duplicateGroup: null, dismissed: true }));
  }
  return r.issues
    .filter((i) => filter === "all" || i.checks.includes(filter))
    .map((i) => ({ ...i, dismissed: false }));
}
```

Run: `pnpm --filter @havenkeys/desktop test -- health` → PASS.

- [ ] **Step 3: Strings (both languages)**

Add a `health` block to `en.ts` (after `generator`) and the same keys to `pt-BR.ts`. Add `health: "Vault health"` to `vault` (sidebar label).

```ts
  health: {
    title: "Vault health",
    intro: "Checked on this device with the vault unlocked. Nothing is sent anywhere.",
    loading: "Checking your logins…",
    empty: "No issues found.",
    emptyFiltered: "Nothing here.",
    all: "All issues",
    dismissedFilter: "Dismissed",
    showItems: "Show items",
    open: "Open",
    changePassword: "Change password",
    howToEnable: "How to enable",
    dismiss: "Dismiss",
    undo: "Undo",
    dismissFailed: "Couldn't save that change.",
    loadFailed: "Couldn't check the vault.",
    cards: {
      reused: { title: "Reused passwords", body: "The same password on several logins. If one site leaks it, the others are exposed." },
      weak: { title: "Weak passwords", body: "Easy to guess. Generate a strong password instead." },
      insecure: { title: "Unsecured websites", body: "Saved with an http:// address. Change it to https:// if the site supports it." },
      duplicate: { title: "Duplicates", body: "Logins with the same name, username and websites. Delete the extra ones." },
      passkey: { title: "Passkeys available", body: "These sites accept passkeys, which can't be phished or reused." },
      two_factor: { title: "Two-factor authentication", body: "These sites offer one-time codes you haven't set up yet." },
      old: { title: "Old passwords", body: "Not changed in over a year." },
    },
    chip: {
      weak: "Weak password",
      reused: (n: number) => `Used in ${n} logins`,
      old: "Not changed in over a year",
      passkey: "Supports passkeys",
      two_factor: "Supports two-factor codes",
      insecure: "Uses http",
      duplicate: (others: number) => (others === 1 ? "Duplicate of 1 other login" : `Duplicate of ${others} other logins`),
    },
  },
```

pt-BR:

```ts
  health: {
    title: "Saúde do cofre",
    intro: "Verificado neste aparelho, com o cofre aberto. Nada é enviado.",
    loading: "Verificando seus logins…",
    empty: "Nenhum problema encontrado.",
    emptyFiltered: "Nada aqui.",
    all: "Todos os problemas",
    dismissedFilter: "Ignorados",
    showItems: "Ver itens",
    open: "Abrir",
    changePassword: "Trocar senha",
    howToEnable: "Como ativar",
    dismiss: "Ignorar",
    undo: "Desfazer",
    dismissFailed: "Não foi possível salvar essa alteração.",
    loadFailed: "Não foi possível verificar o cofre.",
    cards: {
      reused: { title: "Senhas repetidas", body: "A mesma senha em vários logins. Se um site vazar, os outros ficam expostos." },
      weak: { title: "Senhas fracas", body: "Fáceis de adivinhar. Gere uma senha forte no lugar." },
      insecure: { title: "Sites sem segurança", body: "Salvos com um endereço http://. Troque para https:// se o site aceitar." },
      duplicate: { title: "Duplicados", body: "Logins com o mesmo nome, usuário e sites. Apague os que sobram." },
      passkey: { title: "Passkeys disponíveis", body: "Estes sites aceitam passkeys, que não podem ser roubadas por phishing nem repetidas." },
      two_factor: { title: "Verificação em duas etapas", body: "Estes sites oferecem códigos de uso único que você ainda não configurou." },
      old: { title: "Senhas antigas", body: "Não trocadas há mais de um ano." },
    },
    chip: {
      weak: "Senha fraca",
      reused: (n: number) => `Usada em ${n} logins`,
      old: "Não trocada há mais de um ano",
      passkey: "Aceita passkeys",
      two_factor: "Aceita códigos de duas etapas",
      insecure: "Usa http",
      duplicate: (others: number) => (others === 1 ? "Duplicado de 1 outro login" : `Duplicado de ${others} outros logins`),
    },
  },
```

Run: `pnpm --filter @havenkeys/desktop typecheck` → PASS (pt-BR is typed as `Messages`).

- [ ] **Step 4: Write the failing view test**

`apps/desktop/src/views/HealthView.test.tsx` (follow `ExportSection.test.tsx`'s setup: jsdom, `vi.mock("../lib/api")`, `createRoot`, `act`, `I18nProvider` if `ExportSection.test.tsx` wraps with one):

```tsx
// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { HealthCheck, HealthReport, ItemOverview } from "../lib/types";

const setHealthIgnored = vi.fn<(id: string, c: HealthCheck[]) => Promise<void>>();
const openHealthHelp = vi.fn<(id: string, c: HealthCheck) => Promise<void>>();
vi.mock("../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../lib/api")>()),
  api: {
    setHealthIgnored: (id: string, c: HealthCheck[]) => setHealthIgnored(id, c),
    openHealthHelp: (id: string, c: HealthCheck) => openHealthHelp(id, c),
    setUiLanguage: () => Promise.resolve(),
  },
}));

import { en } from "../i18n/en";
import { HealthView } from "./HealthView";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const item = (id: string, title: string): ItemOverview =>
  ({ id, type: "login", title, username: `${title.toLowerCase()}@example.com`, urls: [] }) as unknown as ItemOverview;
// Build ItemOverview the way other desktop tests do if a factory exists (grep "function overview(" in src).

const report: HealthReport = {
  computedAt: 1,
  counts: { weak: 1, reused: 2, old: 0, passkey: 1, twoFactor: 0, insecure: 0, duplicate: 0 },
  issues: [
    { itemId: "a", checks: ["weak", "reused"], reusedGroup: 0, duplicateGroup: null },
    { itemId: "b", checks: ["reused", "passkey"], reusedGroup: 0, duplicateGroup: null },
  ],
  dismissed: [{ itemId: "c", checks: ["old"] }],
};
const items = [item("a", "Alpha"), item("b", "Beta"), item("c", "Gamma")];

let host: HTMLElement;
let root: Root;
const onOpen = vi.fn();
const onEdit = vi.fn();
const onChanged = vi.fn();

function render(r: HealthReport | null, loading = false) {
  act(() =>
    root.render(
      <HealthView items={items} report={r} loading={loading} readOnly={false} onOpen={onOpen} onEdit={onEdit} onChanged={onChanged} />,
    ),
  );
}
const button = (text: string) =>
  Array.from(host.querySelectorAll("button")).find((b) => b.textContent?.includes(text));

beforeEach(() => {
  setHealthIgnored.mockReset().mockResolvedValue(undefined);
  openHealthHelp.mockReset().mockResolvedValue(undefined);
  [onOpen, onEdit, onChanged].forEach((f) => f.mockReset());
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe("HealthView", () => {
  it("shows a card per check with its count and lists titles from the overviews", () => {
    render(report);
    expect(host.textContent).toContain(en.health.cards.reused.title);
    expect(host.textContent).toContain(en.health.chip.reused(2));
    expect(host.textContent).toContain("Alpha");
    expect(host.textContent).not.toContain("Gamma"); // dismissed, hidden until the filter
  });

  it("filters by a card and shows dismissed checks under their filter", () => {
    render(report);
    act(() => button(en.health.cards.passkey.title)?.click());
    expect(host.textContent).toContain("Beta");
    expect(host.textContent).not.toContain("Alpha");
    act(() => button(en.health.dismissedFilter)?.click());
    expect(host.textContent).toContain("Gamma");
  });

  it("dismisses one check and keeps the others", async () => {
    render(report);
    await act(async () => button(en.health.dismiss)?.click());
    expect(setHealthIgnored).toHaveBeenCalledWith("a", expect.arrayContaining(["weak"]));
    expect(onChanged).toHaveBeenCalled();
  });

  it("asks Rust to open the help link by item and check, never by URL", async () => {
    render(report);
    act(() => button(en.health.cards.passkey.title)?.click());
    await act(async () => button(en.health.howToEnable)?.click());
    expect(openHealthHelp).toHaveBeenCalledWith("b", "passkey");
  });

  it("says no issues found, never that the vault is secure", () => {
    render({ ...report, counts: { weak: 0, reused: 0, old: 0, passkey: 0, twoFactor: 0, insecure: 0, duplicate: 0 }, issues: [], dismissed: [] });
    expect(host.textContent).toContain(en.health.empty);
    expect(host.textContent?.toLowerCase()).not.toContain("secure.");
  });

  it("shows the loading state, not a partial report", () => {
    render(null, true);
    expect(host.textContent).toContain(en.health.loading);
  });
});
```

How "Dismiss" chooses which check: a row's Dismiss button dismisses the check of the active filter; under "All issues" each chip carries its own small dismiss control (so the test's first Dismiss button dismisses Alpha's first chip, `weak`). The call sends the row's **existing dismissed checks plus the new one** (read from `report.dismissed`), since the command replaces the list.

Run: `pnpm --filter @havenkeys/desktop test -- HealthView` → FAIL.

- [ ] **Step 5: Implement `HealthChips` and `HealthView`**

Before writing markup, read `apps/desktop/DESIGN.md` §Components (Chips, Cards / Containers, Buttons, Navigation) and reuse the existing classes in `styles.css` (`chip`, `btn`, `btn-small`, `btn-quiet`, `nav-count`, `detail-empty`, `banner`) instead of new ones; add new CSS only for the summary grid, with tokens from `:root` (no raw colors).

`apps/desktop/src/components/HealthChips.tsx`:

```tsx
import type { HealthCheck, HealthReport } from "../lib/types";
import { groupSize } from "../lib/health";
import { useI18n } from "../i18n/context";

export function chipLabel(t: ReturnType<typeof useI18n>["t"], check: HealthCheck, report: HealthReport, itemId: string): string {
  const issue = report.issues.find((i) => i.itemId === itemId);
  switch (check) {
    case "reused":
      return t.health.chip.reused(issue?.reusedGroup != null ? groupSize(report, "reused", issue.reusedGroup) : 2);
    case "duplicate":
      return t.health.chip.duplicate(issue?.duplicateGroup != null ? groupSize(report, "duplicate", issue.duplicateGroup) - 1 : 1);
    default:
      return t.health.chip[check];
  }
}

export function HealthChips({ checks, report, itemId }: { checks: HealthCheck[]; report: HealthReport; itemId: string }) {
  const { t } = useI18n();
  if (checks.length === 0) return null;
  return (
    <ul className="health-chips" aria-label={t.health.title}>
      {checks.map((c) => (
        <li key={c} className={`chip chip-health chip-health-${c}`}>
          {chipLabel(t, c, report, itemId)}
        </li>
      ))}
    </ul>
  );
}
```

`apps/desktop/src/views/HealthView.tsx`: a `<section className="health">` with

- header: `<h1>` `t.health.title`, `<p className="muted">` `t.health.intro`;
- when `loading && !report`: `t.health.loading` with `role="status"`;
- summary grid: one `<button className="health-card" aria-pressed={filter === check}>` per `CHECK_ORDER` entry showing `countOf(report, check)`, `t.health.cards[check].title`, `.body`, and `t.health.showItems`; clicking sets `filter`;
- a filter row with "All issues" and "Dismissed" buttons;
- list `rowsFor(report, filter)` mapped to rows: title and username from `items.find(i => i.id === row.itemId)` (skip a row whose item is gone), `HealthChips`, and buttons: Open (`onOpen(id)`), Change password (`onEdit(id)`, shown when the row has `weak | reused | old`), How to enable (`api.openHealthHelp(id, check)` for `passkey`/`two_factor`), Dismiss / Undo (calls `api.setHealthIgnored(id, next)` then `onChanged()`; disabled when `readOnly`; on error show `toast` with `t.health.dismissFailed` — use `errorMessage` from `i18n/errors.ts` like other views);
- empty: `t.health.empty` when `totalIssues(report) === 0 && filter === "all"`, otherwise `t.health.emptyFiltered`.

No `dangerouslySetInnerHTML`; text only.

- [ ] **Step 6: Wire into `VaultScreen` and `ItemDetail`**

In `VaultScreen.tsx`:
- `Section` type: add `"health"`; `isToolSection` becomes `section === "generator" || section === "settings" || section === "health"`.
- State `const [health, setHealth] = useState<HealthReport | null>(null)` and `const [healthLoading, setHealthLoading] = useState(false)`; `loadHealth()` calls `api.healthReport()` (errors → `null`, no toast unless the view is open, then `t.health.loadFailed`). Call it after every `refresh()` of the items (the same place `setRevision` is bumped) and when `section` becomes `"health"`.
- Sidebar: under Tools, before Generator:

```tsx
<button
  className="nav-item"
  aria-current={section === "health" ? "page" : undefined}
  onClick={() => setSection("health")}
>
  <Icon name="shield" size={17} />
  <span>{t.vault.health}</span>
  {health && totalIssues(health) > 0 && <span className="nav-count">{totalIssues(health)}</span>}
</button>
```

  (If `Icon` has no `shield`, add one in `Icon.tsx` in the same stroke style as the others.)
- Render `{section === "health" && <HealthView items={allItems} report={health} loading={healthLoading} readOnly={readOnly} onOpen={(id) => { setSection("login"); setPane({ kind: "view", id }); }} onEdit={(id) => { setSection("login"); setPane({ kind: "edit", id }); }} onChanged={() => void loadHealth()} />}`.
- Pass `health={health}` to `ItemDetail`.

In `ItemDetail.tsx`: add `health?: HealthReport | null` to `Props`; under the title render `{health && <HealthChips checks={checksFor(health, item.id)} report={health} itemId={item.id} />}`.

On lock, `VaultScreen` unmounts, so the report state goes with it; confirm no `localStorage`/`sessionStorage` write was added (`grep -n "Storage" apps/desktop/src/views/HealthView.tsx apps/desktop/src/lib/health.ts` → nothing).

- [ ] **Step 7: Run tests, typecheck, layout check**

Run: `pnpm --filter @havenkeys/desktop test && pnpm --filter @havenkeys/desktop typecheck && pnpm ui:check`
Expected: PASS; `ui:check` shows no overflow in en or pt-BR (the pt-BR strings are longer).

- [ ] **Step 8: Design review with impeccable**

Invoke the `impeccable:impeccable` skill to critique and polish the Vault health view, the sidebar entry and the item chips against `apps/desktop/DESIGN.md` (Vault Room look, Evergreen Sidebar Rule, chip and card vocabulary, both themes, en and pt-BR). Apply its material fixes, then re-run Step 7.

- [ ] **Step 9: Commit**

```bash
git add apps/desktop/src
git commit -m "feat(desktop): Vault health view, sidebar badge and item chips"
```

---

### Task 8: Mobile FFI

**Files:**
- Create: `crates/havenkeys-mobile/src/health.rs`
- Modify: `crates/havenkeys-mobile/src/lib.rs` (`mod health;` + `pub use health::{HealthKind, HealthView, HealthIssueView, HealthCountsView};`)
- Test: `crates/havenkeys-mobile/tests/health.rs` (or the crate's existing integration-test file layout)

**Interfaces:**
- Consumes: Task 5's `HavenClient::{health_report, set_health_ignored}`, `VaultService::health_help_url`, `MobileVault::{unlocked, send, block_on}`, `items::parse_id`.
- Produces (UniFFI):

```rust
#[derive(uniffi::Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HealthKind { Weak, Reused, Old, Passkey, TwoFactor, Insecure, Duplicate }
#[derive(uniffi::Record)] pub struct HealthCountsView { weak: u32, reused: u32, old: u32, passkey: u32, two_factor: u32, insecure: u32, duplicate: u32 }
#[derive(uniffi::Record)] pub struct HealthIssueView { item_id: String, kinds: Vec<HealthKind>, reused_group: Option<u32>, duplicate_group: Option<u32>, dismissed: bool }
#[derive(uniffi::Record)] pub struct HealthView { counts: HealthCountsView, issues: Vec<HealthIssueView> } // dismissed rows have dismissed = true
MobileVault::health_report() -> MobileResult<HealthView>
MobileVault::set_health_ignored(id: String, kinds: Vec<HealthKind>) -> MobileResult<()>
MobileVault::health_help_url(id: String, kind: HealthKind) -> MobileResult<String>
```

- [ ] **Step 1: Write the failing test**

Following the crate's existing tests (they use `havenkeys_mobile::testing::{seed_with_github_login_unlocked, ...}` from `lib.rs`'s `testing` module), create the test:

```rust
use havenkeys_mobile::{HealthKind};

#[test]
fn health_lists_the_seeded_login_and_dismisses() {
    let v = havenkeys_mobile::testing::vault(); // use the crate's existing constructor helper
    havenkeys_mobile::testing::seed_with_github_login_unlocked(&v);
    let report = v.health_report().unwrap();
    let issue = report.issues.iter().find(|i| !i.dismissed).expect("the github login has an issue");
    // github.com is in the passkey list and the seeded login has no passkey.
    assert!(issue.kinds.contains(&HealthKind::Passkey));
    let url = v.health_help_url(issue.item_id.clone(), HealthKind::Passkey).unwrap();
    assert!(url.starts_with("https://"));
    v.set_health_ignored(issue.item_id.clone(), vec![HealthKind::Passkey]).unwrap();
    let after = v.health_report().unwrap();
    assert!(after.issues.iter().any(|i| i.dismissed && i.kinds == vec![HealthKind::Passkey]));
}

#[test]
fn a_locked_vault_returns_locked() {
    let v = havenkeys_mobile::testing::vault();
    assert_eq!(v.health_report().unwrap_err().code(), "locked"); // match how other tests check MobileError
}
```

Adapt helper names and the error assertion to the ones the crate's other tests use (read one test file first).

Run: `cargo test -p havenkeys-mobile --features testing health` → FAIL.

- [ ] **Step 2: Implement**

```rust
//! Vault health for the phone (spec 2026-10-07-vault-health §8). IDs and
//! check kinds only; the help link is looked up here from the login's own
//! websites, and Kotlin opens it unchanged.

use crate::error::MobileResult;
use crate::items::parse_id;
use crate::vault::MobileVault;
use havenkeys_core::health::{HealthCheck, HealthReport};

#[derive(uniffi::Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HealthKind { Weak, Reused, Old, Passkey, TwoFactor, Insecure, Duplicate }

impl From<HealthCheck> for HealthKind {
    fn from(c: HealthCheck) -> Self {
        match c {
            HealthCheck::Weak => Self::Weak,
            HealthCheck::Reused => Self::Reused,
            HealthCheck::Old => Self::Old,
            HealthCheck::Passkey => Self::Passkey,
            HealthCheck::TwoFactor => Self::TwoFactor,
            HealthCheck::Insecure => Self::Insecure,
            HealthCheck::Duplicate => Self::Duplicate,
        }
    }
}

impl From<HealthKind> for HealthCheck {
    fn from(k: HealthKind) -> Self {
        match k {
            HealthKind::Weak => Self::Weak,
            HealthKind::Reused => Self::Reused,
            HealthKind::Old => Self::Old,
            HealthKind::Passkey => Self::Passkey,
            HealthKind::TwoFactor => Self::TwoFactor,
            HealthKind::Insecure => Self::Insecure,
            HealthKind::Duplicate => Self::Duplicate,
        }
    }
}

#[derive(uniffi::Record)]
pub struct HealthCountsView {
    pub weak: u32, pub reused: u32, pub old: u32, pub passkey: u32,
    pub two_factor: u32, pub insecure: u32, pub duplicate: u32,
}

#[derive(uniffi::Record)]
pub struct HealthIssueView {
    pub item_id: String,
    pub kinds: Vec<HealthKind>,
    pub reused_group: Option<u32>,
    pub duplicate_group: Option<u32>,
    pub dismissed: bool,
}

#[derive(uniffi::Record)]
pub struct HealthView {
    pub counts: HealthCountsView,
    pub issues: Vec<HealthIssueView>,
}

fn view(r: HealthReport) -> HealthView {
    let c = r.counts;
    let mut issues: Vec<HealthIssueView> = r.issues.into_iter().map(|i| HealthIssueView {
        item_id: i.item_id.to_string(),
        kinds: i.checks.into_iter().map(Into::into).collect(),
        reused_group: i.reused_group,
        duplicate_group: i.duplicate_group,
        dismissed: false,
    }).collect();
    issues.extend(r.dismissed.into_iter().map(|d| HealthIssueView {
        item_id: d.item_id.to_string(),
        kinds: d.checks.into_iter().map(Into::into).collect(),
        reused_group: None,
        duplicate_group: None,
        dismissed: true,
    }));
    HealthView {
        counts: HealthCountsView {
            weak: c.weak, reused: c.reused, old: c.old, passkey: c.passkey,
            two_factor: c.two_factor, insecure: c.insecure, duplicate: c.duplicate,
        },
        issues,
    }
}

#[uniffi::export]
impl MobileVault {
    /// Blocking and slow on a large vault: call on `Dispatchers.IO`.
    pub fn health_report(&self) -> MobileResult<HealthView> {
        self.unlocked()?;
        Ok(view(self.client.health_report(havenkeys_client::now_ms())?))
    }

    /// Replaces the login's dismissed checks (send the full list).
    pub fn set_health_ignored(&self, id: String, kinds: Vec<HealthKind>) -> MobileResult<()> {
        let id = parse_id(&id)?;
        self.unlocked()?;
        let client = self.client.clone();
        self.block_on(client.set_health_ignored(id, kinds.into_iter().map(Into::into).collect()))?;
        havenkeys_client::ClientEvents::items_changed(&*self.events);
        Ok(())
    }

    pub fn health_help_url(&self, id: String, kind: HealthKind) -> MobileResult<String> {
        let id = parse_id(&id)?;
        self.unlocked()?;
        Ok(self.client.vault()?.health_help_url(&id, kind.into())?)
    }
}
```

Add `mod health;` and the `pub use` line to `lib.rs`.

- [ ] **Step 3: Run the tests and regenerate bindings**

Run: `cargo test -p havenkeys-mobile --features testing && cargo clippy -p havenkeys-mobile --all-targets --features testing -- -D warnings && scripts/build-android.sh`
Expected: PASS; the Kotlin bindings under `apps/android` change (commit them, as `docs/development.md` says).

- [ ] **Step 4: Commit**

```bash
git add -A crates/havenkeys-mobile apps/android   # includes the regenerated Kotlin bindings
git commit -m "feat(mobile): vault health API"
```

---

### Task 9: Android — Health screen, Home card, item chips

**Files:**
- Modify: `apps/android/app/src/main/kotlin/net/havenkeys/android/data/VaultRepository.kt`
- Modify: `apps/android/app/src/test/kotlin/net/havenkeys/android/fakes/FakeRepositories.kt`
- Create: `apps/android/app/src/main/kotlin/net/havenkeys/android/ui/health/HealthViewModel.kt`, `HealthScreen.kt`, `HealthChips.kt`
- Create: `apps/android/app/src/test/kotlin/net/havenkeys/android/ui/health/HealthViewModelTest.kt`
- Modify: `ui/home/HomeViewModel.kt`, `ui/home/HomeScreen.kt`, `ui/item/ItemViewModel.kt`, `ui/item/ItemScreen.kt`, `ui/nav/Routes.kt`, `ui/nav/HavenNavHost.kt`
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`
- Modify: `test/.../ui/home/HomeViewModelTest.kt`

**Interfaces:**
- Consumes: Task 8's `HealthView`, `HealthIssueView`, `HealthKind`, `MobileVault.healthReport/setHealthIgnored/healthHelpUrl`.
- Produces: `VaultRepository.health(): Outcome<HealthView>`, `setHealthIgnored(id: String, kinds: List<HealthKind>): Outcome<Unit>`, `healthHelpUrl(id: String, kind: HealthKind): Outcome<String>`; `Routes.HEALTH = "health"`; `HealthUiState`; `HomeUiState.healthTotal: Int?`; `ItemUiState` gains `health: List<HealthKind>`.

- [ ] **Step 1: Repository and fake**

`VaultRepository` interface:

```kotlin
    suspend fun health(): Outcome<HealthView>
    suspend fun setHealthIgnored(id: String, kinds: List<HealthKind>): Outcome<Unit>
    suspend fun healthHelpUrl(id: String, kind: HealthKind): Outcome<String>
```

`RustVaultRepository`:

```kotlin
    override suspend fun health() = rust { vault.healthReport() }
    override suspend fun setHealthIgnored(id: String, kinds: List<HealthKind>) = rust { vault.setHealthIgnored(id, kinds) }
    override suspend fun healthHelpUrl(id: String, kind: HealthKind) = rust { vault.healthHelpUrl(id, kind) }
```

`FakeVaultRepository`:

```kotlin
    var healthView: Outcome<HealthView> = Outcome.Ok(HealthView(HealthCountsView(0u, 0u, 0u, 0u, 0u, 0u, 0u), emptyList()))
    var helpUrl: Outcome<String> = Outcome.Failed("not_found")

    override suspend fun health(): Outcome<HealthView> {
        calls += "health"
        return healthView
    }

    override suspend fun setHealthIgnored(id: String, kinds: List<HealthKind>): Outcome<Unit> {
        calls += "ignore:$id:${kinds.joinToString(",")}"
        return Outcome.Ok(Unit)
    }

    override suspend fun healthHelpUrl(id: String, kind: HealthKind): Outcome<String> {
        calls += "help:$id:$kind"
        return helpUrl
    }
```

- [ ] **Step 2: Write the failing ViewModel test**

`HealthViewModelTest.kt`:

```kotlin
package net.havenkeys.android.ui.health

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeVaultRepository
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.HealthCountsView
import uniffi.havenkeys_mobile.HealthIssueView
import uniffi.havenkeys_mobile.HealthKind
import uniffi.havenkeys_mobile.HealthView
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@OptIn(ExperimentalCoroutinesApi::class)
class HealthViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())
    @After fun reset() = Dispatchers.resetMain()

    private val events = VaultEventsHub()
    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(listOf(summary("a", "Alpha"), summary("b", "Beta"), summary("c", "Gamma")))
        healthView = Outcome.Ok(
            HealthView(
                HealthCountsView(1u, 2u, 0u, 1u, 0u, 0u, 0u),
                listOf(
                    HealthIssueView("a", listOf(HealthKind.WEAK, HealthKind.REUSED), 0u, null, false),
                    HealthIssueView("b", listOf(HealthKind.REUSED, HealthKind.PASSKEY), 0u, null, false),
                    HealthIssueView("c", listOf(HealthKind.OLD), null, null, true),
                ),
            ),
        )
    }

    private fun vm() = HealthViewModel(vault, events)

    @Test
    fun loadsRowsWithTitlesFromTheOverviews() {
        val vm = vm()
        vm.shown()
        val rows = vm.state.value.rows
        assertEquals(listOf("Alpha", "Beta"), rows.map { it.title })
        assertEquals(4, vm.state.value.total)
    }

    @Test
    fun filtersByKindAndShowsDismissed() {
        val vm = vm()
        vm.shown()
        vm.filter(HealthFilter.Kind(HealthKind.PASSKEY))
        assertEquals(listOf("Beta"), vm.state.value.rows.map { it.title })
        vm.filter(HealthFilter.Dismissed)
        assertEquals(listOf("Gamma"), vm.state.value.rows.map { it.title })
    }

    @Test
    fun dismissSendsTheFullListAndReloads() {
        val vm = vm()
        vm.shown()
        vm.dismiss("c", HealthKind.WEAK)
        assertTrue("ignore:c:OLD,WEAK" in vault.calls)
        assertEquals(2, vault.calls.count { it == "health" })
    }

    @Test
    fun lockWipesTheState() {
        val vm = vm()
        vm.shown()
        events.locked("user")
        assertTrue(vm.state.value.rows.isEmpty())
        assertNull(vm.state.value.counts)
    }

    private fun summary(id: String, title: String) =
        ItemSummary(id, ItemKind.LOGIN, title, null, null, false, false, 0, 0)
}
```

`summary` mirrors `HomeViewModelTest.item(...)`; `VaultEventsHub.locked(reason)` is how the Rust callback reports a lock. Remove the unused `VaultEvent` import if detekt flags it. Note the dismiss call sends the existing dismissed kinds plus the new one, sorted by enum order (`OLD,WEAK`).

Run: `cd apps/android && ./gradlew testGithubDebugUnitTest --tests '*HealthViewModelTest*'` → FAIL.

- [ ] **Step 3: Implement the ViewModel**

```kotlin
package net.havenkeys.android.ui.health

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.HealthCountsView
import uniffi.havenkeys_mobile.HealthIssueView
import uniffi.havenkeys_mobile.HealthKind

sealed interface HealthFilter {
    data object All : HealthFilter
    data object Dismissed : HealthFilter
    data class Kind(val kind: HealthKind) : HealthFilter
}

/** One row: the title and username from the overview, never a value. */
data class HealthRow(
    val id: String,
    val title: String,
    val subtitle: String?,
    val kinds: List<HealthKind>,
    val groupSize: Int?,
    val duplicates: Int?,
    val dismissed: Boolean,
)

data class HealthUiState(
    val counts: HealthCountsView? = null,
    val total: Int = 0,
    val filter: HealthFilter = HealthFilter.All,
    val rows: List<HealthRow> = emptyList(),
    val loading: Boolean = true,
    val errorCode: String? = null,
)

/** Watchtower's order, then "old". */
val KIND_ORDER = listOf(
    HealthKind.REUSED, HealthKind.WEAK, HealthKind.INSECURE, HealthKind.DUPLICATE,
    HealthKind.PASSKEY, HealthKind.TWO_FACTOR, HealthKind.OLD,
)

fun HealthCountsView.of(kind: HealthKind): Int = when (kind) {
    HealthKind.WEAK -> weak
    HealthKind.REUSED -> reused
    HealthKind.OLD -> old
    HealthKind.PASSKEY -> passkey
    HealthKind.TWO_FACTOR -> twoFactor
    HealthKind.INSECURE -> insecure
    HealthKind.DUPLICATE -> duplicate
}.toInt()

class HealthViewModel(private val vault: VaultRepository, events: VaultEventsHub) : ViewModel() {
    private val _state = MutableStateFlow(HealthUiState())
    val state: StateFlow<HealthUiState> = _state.asStateFlow()
    private var issues: List<HealthIssueView> = emptyList()
    private var titles: Map<String, Pair<String, String?>> = emptyMap()
    private var pending: Job? = null

    init {
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> wipe()
                    VaultEvent.ItemsChanged -> load()
                    else -> Unit
                }
            }
        }
    }

    fun shown() = load()

    fun filter(f: HealthFilter) {
        _state.update { it.copy(filter = f, rows = rows(f)) }
    }

    fun dismiss(id: String, kind: HealthKind) = change(id) { it + kind }

    fun undo(id: String, kind: HealthKind) = change(id) { it - kind }

    suspend fun helpUrl(id: String, kind: HealthKind): String? = (vault.healthHelpUrl(id, kind) as? Outcome.Ok)?.value

    private fun change(id: String, edit: (Set<HealthKind>) -> Set<HealthKind>) {
        viewModelScope.launch {
            val current = issues.filter { it.itemId == id && it.dismissed }.flatMap { it.kinds }.toSet()
            val next = edit(current).sortedBy { it.ordinal }
            when (val r = vault.setHealthIgnored(id, next)) {
                is Outcome.Failed -> _state.update { it.copy(errorCode = r.code) }
                is Outcome.Ok -> load()
            }
        }
    }

    private fun load() {
        pending?.cancel()
        pending = viewModelScope.launch {
            val all = vault.list()
            val report = vault.health()
            if (report is Outcome.Failed) {
                _state.update { it.copy(loading = false, errorCode = report.code) }
                return@launch
            }
            val view = (report as Outcome.Ok).value
            titles = (all as? Outcome.Ok)?.value.orEmpty().associate { it.id to (it.title to it.subtitle) }
            issues = view.issues
            val total = KIND_ORDER.sumOf { view.counts.of(it) }
            _state.update {
                it.copy(counts = view.counts, total = total, rows = rows(it.filter), loading = false, errorCode = null)
            }
        }
    }

    private fun rows(f: HealthFilter): List<HealthRow> = issues
        .filter {
            when (f) {
                HealthFilter.All -> !it.dismissed
                HealthFilter.Dismissed -> it.dismissed
                is HealthFilter.Kind -> !it.dismissed && f.kind in it.kinds
            }
        }
        .mapNotNull { i ->
            val (title, subtitle) = titles[i.itemId] ?: return@mapNotNull null
            HealthRow(
                id = i.itemId,
                title = title,
                subtitle = subtitle,
                kinds = i.kinds,
                groupSize = i.reusedGroup?.let { g -> issues.count { it.reusedGroup == g && !it.dismissed } },
                duplicates = i.duplicateGroup?.let { g -> issues.count { it.duplicateGroup == g && !it.dismissed } - 1 },
                dismissed = i.dismissed,
            )
        }

    private fun wipe() {
        pending?.cancel()
        issues = emptyList()
        titles = emptyMap()
        _state.value = HealthUiState()
    }
}
```

Field names on `ItemSummary` (`title`, `subtitle`) must match the generated bindings; adjust if the summary uses `username`. Register the ViewModel where the others are created (see `NavServices.kt` / the factory used by `HomeViewModel`).

Run the test → PASS.

- [ ] **Step 4: Home card and item chips**

`HomeViewModel`: add `val healthTotal: Int? = null` to `HomeUiState`; in `load()` also call `vault.health()` and set `healthTotal = KIND_ORDER.sumOf { view.counts.of(it) }` (null on failure). Add a test in `HomeViewModelTest`: `shownLoadsTheHealthTotal` (fake returns counts 1,2,0,1,0,0,0 → `healthTotal == 4`; `"health" in vault.calls`).

`ItemViewModel`: after loading the view, if the item is a login, call `vault.health()` and keep `health = issues.firstOrNull { it.itemId == id && !it.dismissed }?.kinds.orEmpty()` in its UI state; wipe on lock like the rest of its state.

`Routes.kt`: `const val HEALTH = "health"`. `HavenNavHost.kt`: a `composable(Routes.HEALTH)` for `HealthScreen`, with `onOpen = { navController.navigate(Routes.item(it)) }`, `onEdit = { navController.navigate(Routes.edit(it)) }`.

- [ ] **Step 5: Strings**

`res/values/strings.xml` (and the same names in `values-pt-rBR/strings.xml` with the pt-BR text from Task 7 Step 3):

```xml
    <string name="health_title">Vault health</string>
    <string name="health_intro">Checked on this phone with the vault unlocked. Nothing is sent anywhere.</string>
    <string name="health_loading">Checking your logins…</string>
    <string name="health_empty">No issues found.</string>
    <string name="health_empty_filtered">Nothing here.</string>
    <string name="health_all">All issues</string>
    <string name="health_dismissed">Dismissed</string>
    <string name="health_open">Open</string>
    <string name="health_change_password">Change password</string>
    <string name="health_how_to_enable">How to enable</string>
    <string name="health_dismiss">Dismiss</string>
    <string name="health_undo">Undo</string>
    <string name="health_home_card">%1$d issues to review</string>
    <string name="health_reused_title">Reused passwords</string>
    <string name="health_weak_title">Weak passwords</string>
    <string name="health_insecure_title">Unsecured websites</string>
    <string name="health_duplicate_title">Duplicates</string>
    <string name="health_passkey_title">Passkeys available</string>
    <string name="health_two_factor_title">Two-factor authentication</string>
    <string name="health_old_title">Old passwords</string>
    <string name="health_chip_weak">Weak password</string>
    <plurals name="health_chip_reused"><item quantity="one">Used in %1$d login</item><item quantity="other">Used in %1$d logins</item></plurals>
    <string name="health_chip_old">Not changed in over a year</string>
    <string name="health_chip_passkey">Supports passkeys</string>
    <string name="health_chip_two_factor">Supports two-factor codes</string>
    <string name="health_chip_insecure">Uses http</string>
    <plurals name="health_chip_duplicate"><item quantity="one">Duplicate of %1$d other login</item><item quantity="other">Duplicate of %1$d other logins</item></plurals>
```

(Use the card body strings from Task 7 as `health_<kind>_body` too.) pt-BR `health_home_card`: `%1$d problemas para revisar`.

- [ ] **Step 6: Compose screens**

Before writing, read `apps/android/DESIGN.md` and reuse the existing kit components (`ui/kit`, `ui/components`): inset groups of hairline rows, serif titles, the brass accent only once per screen, no Material components. Build:

- `HealthChips(kinds, groupSize, duplicates)`: a `FlowRow` of the kit's chip/tag component with the strings above.
- `HealthScreen(vm, onOpen, onEdit, onBack)`: top bar with back and `health_title`; intro line; a horizontally scrolling row (or 2-column grid) of the seven kind cards with counts, each selecting `HealthFilter.Kind`; "All issues" / "Dismissed" toggles; a `LazyColumn` of rows (title, subtitle, `HealthChips`) whose overflow menu has Open, Change password (weak/reused/old), How to enable (passkey/two-factor: `vm.helpUrl(...)` then `context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))`, with the URL used unchanged and only when non-null), Dismiss / Undo; loading and empty states with the strings above. Call `vm.shown()` in a `LaunchedEffect(Unit)`.
- Home: one row/card "Vault health" showing `health_home_card` when `healthTotal > 0` (else `health_empty`), navigating to `Routes.HEALTH`, placed with Home's other groups using its `Settle` sequencing.
- Item screen: `HealthChips` under the title when `state.health` is not empty.

- [ ] **Step 7: Run Android checks**

Run: `cd apps/android && ./gradlew detekt testGithubDebugUnitTest lintGithubDebug`
Expected: PASS (string parity between `values` and `values-pt-rBR` is enforced by lint/tests).

- [ ] **Step 8: Design review with impeccable**

Invoke the `impeccable:impeccable` skill to critique and polish the Health screen, Home card and item chips against `apps/android/DESIGN.md` (forest grounds, one brass fitting, serif titles, inset hairline groups, thumb reach, dark and light, en and pt-BR). Apply its material fixes and re-run Step 7.

- [ ] **Step 9: Commit**

```bash
git add apps/android
git commit -m "feat(android): Vault health screen, Home card and item chips"
```

---

### Task 10: Documentation, sizes and the security review entry

**Files:**
- Modify: `docs/security-model.md`, `docs/architecture.md`, `docs/security-review.md`, `docs/ideas.md`

- [ ] **Step 1: Measure the size cost**

Run (record numbers in the review entry):

```bash
scripts/build-android.sh --release && ls -l target/aarch64-linux-android/release/libhavenkeys_mobile.so
cargo build --release -p havenkeys-desktop && ls -l target/release/havenkeys-desktop
```

Then build the same two artifacts from the commit before this feature (`git worktree add <scratch>/base 8b90b93`, run the same commands there) and record before/after sizes. If the desktop build needs WebKit and it is not available, measure `libhavenkeys_mobile.so` only and say so in the review entry.

- [ ] **Step 2: Audits**

Run: `cargo audit && cargo deny check && pnpm audit --prod`
Expected: nothing new versus `main`. Investigate anything new before continuing.

- [ ] **Step 3: Write the docs**

- `docs/security-model.md`: a "Vault health" section: computed in Rust on the unlocked vault; the report holds item IDs, check kinds, group indexes and counts (no password, score, length, hash, title, username or URL); cached in the session, dropped on lock, recomputed after any change or 10 minutes; passwords read into a snapshot under the vault lock and released after compute; zxcvbn copies up to 100 characters of each password and drops its match list without zeroizing (same limitation as other heap strings, §9); help links come from the bundled directories, https only, opened on click; dismissals are encrypted item data that sync; no network access, no third party.
- `docs/architecture.md`: the `health` module, the two bundled data files and their update scripts, and the client's off-lock compute.
- `docs/security-review.md`: a new entry "Vault health" (findings/severity/component/attack scenario/mitigation/remaining limitations) covering: report content (no secrets), lock/epoch race (stale report not cached), directory poisoning (reviewed data, https-only links, label matching), zxcvbn copies (low; accepted, documented), size cost (numbers from Step 1), and HIBP explicitly not implemented.
- `docs/ideas.md` item 1: mark "Especificada em `docs/superpowers/specs/2026-10-07-vault-health-design.md`; implementada na versão seguinte à 0.19.0" and fix the note: the passkey list's `mfa` means "passkey as a second factor", not TOTP; the TOTP sites come from `2factorauth/twofactorauth`. (This file was untracked at the start; add it to git only if the owner agrees — otherwise edit it and leave it untracked.)

- [ ] **Step 4: Full verification**

Run: `pnpm typecheck && pnpm -r test && cargo test && pnpm lint:rust && cargo test -p havenkeys-mobile --features testing && (cd apps/android && ./gradlew detekt testGithubDebugUnitTest lintGithubDebug)`
Expected: everything passes. Report any failure with its output instead of claiming success.

- [ ] **Step 5: Commit**

```bash
git add docs/security-model.md docs/architecture.md docs/security-review.md
git commit -m "docs: vault health in the security model, architecture and security review"
```
