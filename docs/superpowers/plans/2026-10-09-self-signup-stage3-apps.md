# Self-Service Signup — Stage 3: Apps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The desktop app, the browser extension and the Android app learn the account's plan from the server, show the trial, and honour a frozen account: reads, TOTP, export and passkey sign-in keep working; writes, autofill, saving and passkey creation stop, decided in Rust from the stored entitlement. Onboarding gains "Create account" and "I have a setup code".

**Architecture:** The sync client parses the `account` object the server now sends on login and sync and maps `402` to `SyncError::AccountFrozen`. The core store keeps the plan in four plaintext columns of the `account` table (schema 7) behind a `PlanRecord`; `VaultService::entitlement()` reads it. `HavenClient::require_full()` gates every write path (`push`, `push_batches`, the master-password change) and the shells call it before staging. The bridge refuses the autofill requests with a new `ErrorCode::Frozen` and tells the extension the entitlement in `status` and `find_matches`, so the popup can offer TOTP and "Open in desktop" while the inline menu stays out of the page. The desktop and Android shells show "Trial: N days left" or a frozen banner with Subscribe, and disable the editors.

**Tech Stack:** Rust (havenkeys-sync-client, havenkeys-core, havenkeys-client, havenkeys-bridge, havenkeys-protocol, havenkeys-mobile with UniFFI), Tauri 2 + React/TypeScript (vitest, jsdom), WebExtension TypeScript (vitest), Kotlin/Compose (JUnit4, Robolectric).

**Spec:** `docs/superpowers/specs/2026-10-07-self-signup-and-plans-design.md` §5.4, §6, §8 (client bullets). Index: `docs/superpowers/plans/2026-10-09-self-signup-index.md`. Server contract (stage 1, done): `POST /v1/auth/login` and `GET /v1/sync` carry `"account": { "status", "entitlement", "trialEndsAt", "periodEnd" }` (`status` ∈ trialing, active, past_due, frozen, complimentary; `entitlement` ∈ full, frozen; dates RFC 3339 or null); `402 {"error":{"code":"account_frozen"}}` on `POST /v1/items`, `POST /v1/account/credentials`, pairing approval and a never-seen device's login.

## Global Constraints

- Subscribe and "Create account" open fixed URLs, verbatim: `https://havenkeys.net/pricing` and `https://havenkeys.net/signup`. (Ruling: the spec's `/account` page does not exist; stage 2 builds `/pricing`.)
- The frozen popup offers the current site's TOTP code and "Open in desktop"; there is no new reveal request. (Ruling: spec §6.3 says the popup *may* show the password; a new secret path through the extension is not worth it when the desktop reveals and copies.)
- Rust decides from the stored entitlement; no UI decides on its own. Refusals: the client core's `require_full()` (code `account_frozen`) and the bridge's `ErrorCode::Frozen` (wire `frozen`, message verbatim: `Your HavenKeys trial has ended. The vault is read-only until you subscribe.`).
- Still allowed while frozen: open, search, reveal, copy, TOTP, export, delete account, device list and revoke, logout, lock, passkey sign-in, "Open in desktop". Stopped: create, edit, trash, restore, purge, import, restore backup, change master password, inline menu, Fill, automatic sign-in, sign-in-with, save prompts, generator insert (through the menu), cards and identity fill, Android system autofill datasets and saving, passkey creation.
- Attack 1 holds when frozen: `fill_item` on a wrong origin answers `denied`, never `frozen`; `get_totp` stays origin-bound.
- The plan is non-secret metadata: it lives in plaintext columns of the local `account` table and is readable while locked (for the welcome/unlock screens' banners).
- A `402` from the server also flips the stored entitlement to `frozen` at once, so the UI reflects it before the next sync; the next sync's `account` object is authoritative.
- Every user-facing string exists in EN and PT-BR (desktop `en.ts`/`pt-BR.ts`, extension `en.ts`/`pt-BR.ts`, Android `values`/`values-pt-rBR`). Product names stay English.
- Store schema: `SCHEMA_VERSION` 6 → 7 with migrations for 5 and 6; a vault at 7 opens; older-than-5 stays `UnsupportedVersion`.
- The native host is relay-only but validates frames against `havenkeys-protocol`, so it must be rebuilt and re-packaged with the extension (`pnpm package:extension`).
- No commit carries a `Co-Authored-By` trailer. Formatting: the branch base is not rustfmt-clean; format only touched files.
- Tests: `cargo test -p havenkeys-sync-client -p havenkeys-core -p havenkeys-client -p havenkeys-bridge -p havenkeys-protocol -p havenkeys-mobile` (Postgres on 5433 for the round trips), `pnpm --filter @havenkeys/protocol test`, `pnpm --filter @havenkeys/extension test`, `pnpm --filter @havenkeys/desktop test`, `cargo test -p havenkeys-desktop` (Tauri crate), Android `./gradlew :app:testDebugUnitTest` with the JAVA_HOME/ANDROID_NDK_HOME from the owner's memory notes.

## Review Focus

1. **A device that was frozen and gets a sync saying `full` again** (operator set the plan, or payment landed) must unfreeze without a restart: the stored entitlement is overwritten on every sync page and the UI re-reads it on the plan-changed event. (Task 3: `a_sync_that_says_full_unfreezes`; Task 6 and Task 7 listen to the event.)
2. **An old server that sends no `account` object** (self-hosted, not yet updated) must leave the client Full, not frozen. (Task 1: `a_login_without_an_account_object_is_full`; Task 3 treats `None` as "keep the stored value, default Full".)
3. **Frozen while locked**: the unlock screen and the welcome screen must not crash or show stale copy; the plan columns are readable while locked. (Task 2: `plan_is_readable_while_locked`.)
4. **The 402 on a background write** (`purge_expired_trash`, `ensure_identity`) must not surface as a user error but must flip the stored entitlement. (Task 3: `a_402_marks_the_account_frozen`.)
5. **Android autofill with a frozen account** must answer with no datasets quickly and must not show "Save password?". (Task 7: `HavenAutofillServiceTest`-level check through `AutofillRepository.frozen()`; the save path returns `account_frozen` and the toast says why.)

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/havenkeys-sync-client/src/wire.rs` | `AccountDto` on `LoginDto`, `PullDto` |
| `crates/havenkeys-sync-client/src/session.rs`, `client.rs`, `error.rs` | `AccountInfo`, `Session.account`, `Pulled.account`, `SyncError::AccountFrozen`, 402 mapping |
| `crates/havenkeys-core/src/store.rs` | schema 7, `PlanRecord`, `Entitlement`, `plan()`, `set_plan()` |
| `crates/havenkeys-core/src/vault.rs` | `VaultService::{plan, set_plan, entitlement}` |
| `crates/havenkeys-client/src/{error,events,sync,client,account}.rs` | `account_frozen`, `plan_changed` event, `require_full`, persistence, `AccountStatus` fields, `preview_invite` |
| `crates/havenkeys-protocol/src/message.rs` | `ErrorCode::Frozen`, `entitlement` on `Status` and `FindMatches` |
| `packages/protocol/src/index.ts` | the same on the TS side |
| `crates/havenkeys-bridge/src/dispatch.rs` + `tests/bridge.rs` | the frozen gate |
| `apps/extension/src/{messaging/popup.ts,background/popup-handler.ts,popup/popup.ts,background/inline-handler.ts,i18n/*}` | frozen popup, menu stays out |
| `apps/desktop/src-tauri/src/{account,events,commands}.rs`, `build.rs`, `capabilities/main.json`, `lib.rs` | `preview_invite`, `open_pricing`, `open_signup`, plan event |
| `apps/desktop/src/{App.tsx,views/WelcomeScreen.tsx,views/VaultScreen.tsx,lib/api.ts,lib/types.ts,i18n/*}` | UI |
| `crates/havenkeys-mobile/src/{vault,onboarding,events}.rs` | `Status` fields, `preview_invite`, `frozen()`, event |
| `apps/android/app/src/main/kotlin/net/havenkeys/android/{autofill,ui/onboarding,ui/settings,data,ui/components}` + `res/values*/strings.xml` | UI and autofill |
| `docs/native-messaging.md`, `docs/security-model.md`, `docs/android.md`, `docs/security-review.md` | docs |

---

### Task 1: Sync client — the `account` object and `402`

**Files:**
- Modify: `crates/havenkeys-sync-client/src/wire.rs` (`LoginDto`, `PullDto`)
- Modify: `crates/havenkeys-sync-client/src/session.rs` (`Session.account`)
- Modify: `crates/havenkeys-sync-client/src/client.rs` (`Pulled`, `login`, `pull`, `error_from`, `error_for`)
- Modify: `crates/havenkeys-sync-client/src/error.rs` (`SyncError::AccountFrozen`)
- Modify: `crates/havenkeys-sync-client/src/lib.rs` (export `AccountInfo`, `Entitlement`)
- Test: `crates/havenkeys-sync-client/src/client.rs` unit tests (if the file has a `mod tests`; otherwise `tests/wire.rs`), `crates/havenkeys-sync-client/tests/round_trip.rs`

**Interfaces:**
- Produces:
  - `pub enum Entitlement { Full, Frozen }` (in `havenkeys-sync-client`, re-exported; `Copy, Clone, Debug, PartialEq, Eq`).
  - `pub struct AccountInfo { pub status: String, pub entitlement: Entitlement, pub trial_ends_at: Option<String>, pub period_end: Option<String> }` (`Clone, Debug, PartialEq, Eq`).
  - `Session.account: Option<AccountInfo>` (public field, `None` from `Session::new`).
  - `Pulled.account: Option<AccountInfo>`.
  - `SyncError::AccountFrozen` with `code()` = `"account_frozen"`, Display `"this account is frozen: the trial ended or payment lapsed"`.

- [ ] **Step 1: Write the failing tests**

In `crates/havenkeys-sync-client/src/client.rs`, add to the existing test module (create `#[cfg(test)] mod tests` at the end of the file if there is none, with `use super::*;`):

```rust
    #[test]
    fn error_for_maps_402_to_frozen_only_when_the_server_says_so() {
        let said = HttpResponse {
            status: 402,
            body: br#"{"error":{"code":"account_frozen","message":"x"}}"#.to_vec(),
        };
        assert_eq!(error_from(&said), SyncError::AccountFrozen);
        let other = HttpResponse {
            status: 402,
            body: br#"{"error":{"code":"something_else","message":"x"}}"#.to_vec(),
        };
        assert_eq!(error_from(&other), SyncError::Refused("the request was refused"));
        assert_eq!(SyncError::AccountFrozen.code(), "account_frozen");
    }

    #[test]
    fn an_account_object_is_read_and_its_absence_is_tolerated() {
        let with: wire::LoginDto = wire::parse(
            br#"{"token":"t","expiresAt":"2026-10-10T00:00:00Z","vaultId":"00000000-0000-0000-0000-000000000001",
                 "account":{"status":"trialing","entitlement":"frozen","trialEndsAt":"2026-10-09T00:00:00Z","periodEnd":null}}"#,
        )
        .unwrap();
        let info = AccountInfo::from(with.account.unwrap());
        assert_eq!(info.status, "trialing");
        assert_eq!(info.entitlement, Entitlement::Frozen);
        assert_eq!(info.trial_ends_at.as_deref(), Some("2026-10-09T00:00:00Z"));
        assert_eq!(info.period_end, None);
        let without: wire::LoginDto = wire::parse(
            br#"{"token":"t","expiresAt":"2026-10-10T00:00:00Z","vaultId":"00000000-0000-0000-0000-000000000001"}"#,
        )
        .unwrap();
        assert!(without.account.is_none());
        // An unknown entitlement word is Full: a newer server must not freeze an older client by accident.
        let odd: wire::AccountDto =
            wire::parse(br#"{"status":"weird","entitlement":"paused","trialEndsAt":null,"periodEnd":null}"#).unwrap();
        assert_eq!(AccountInfo::from(odd).entitlement, Entitlement::Full);
    }
```

If `HttpResponse` fields are named differently in `client.rs`, use its real constructor; the shape is `status: u16, body: Vec<u8>`.

Add to `crates/havenkeys-sync-client/tests/round_trip.rs` (it already has a server fixture and a signed-in client; follow its helpers):

```rust
#[tokio::test]
async fn login_and_pull_carry_the_account_and_a_frozen_write_is_named() {
    // Use the file's existing helpers to start a server, create an account
    // (`trial: false` keeps it complimentary) and build a `ServerClient`.
    let (server, client, session) = signed_in_fixture().await;
    assert_eq!(
        session.account.as_ref().map(|a| a.entitlement),
        Some(Entitlement::Full)
    );
    let pulled = client.pull(&session, 0).await.unwrap();
    assert_eq!(pulled.account.as_ref().map(|a| a.status.as_str()), Some("complimentary"));

    havenkeys_server::billing::set_status(
        &server.db().await, session.account_id,
        havenkeys_server::billing::Actor::Admin,
        havenkeys_server::billing::Status::Frozen, None, "test",
    ).await.unwrap();
    let err = client.write(&session, &[staged_item()]).await.unwrap_err();
    assert_eq!(err, SyncError::AccountFrozen);
    let pulled = client.pull(&session, 0).await.unwrap();
    assert_eq!(pulled.account.as_ref().map(|a| a.entitlement), Some(Entitlement::Frozen));
    server.cleanup().await;
}
```

`signed_in_fixture()` and `staged_item()` stand for whatever that test file already uses to get a logged-in `ServerClient` with a `Session` and to build one `StagedWrite`; name them as the file does. `havenkeys_server` is already a dev-dependency of this crate (the file imports its admin CLI).

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p havenkeys-sync-client`
Expected: compile errors (`account` field, `AccountInfo`, `Entitlement`, `AccountFrozen`).

- [ ] **Step 3: The wire types**

In `crates/havenkeys-sync-client/src/wire.rs`:

```rust
/// The plan the server reports (spec 2026-10-07 §5.4). Every field is
/// optional on the wire: a server from before plans sends none.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub status: String,
    pub entitlement: String,
    #[serde(default)]
    pub trial_ends_at: Option<String>,
    #[serde(default)]
    pub period_end: Option<String>,
}
```

Add `#[serde(default)] pub account: Option<AccountDto>,` to both `LoginDto` and `PullDto`.

- [ ] **Step 4: The public types and the mapping**

In `crates/havenkeys-sync-client/src/session.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entitlement {
    Full,
    Frozen,
}

/// The account's plan as the server last reported it. Not secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountInfo {
    pub status: String,
    pub entitlement: Entitlement,
    pub trial_ends_at: Option<String>,
    pub period_end: Option<String>,
}

impl From<crate::wire::AccountDto> for AccountInfo {
    fn from(dto: crate::wire::AccountDto) -> Self {
        // Only the one word the client acts on freezes it; anything else,
        // including a word a newer server might add, reads as Full.
        let entitlement = if dto.entitlement == "frozen" {
            Entitlement::Frozen
        } else {
            Entitlement::Full
        };
        let short = |s: String| if s.chars().count() > 32 { String::new() } else { s };
        Self {
            status: short(dto.status),
            entitlement,
            trial_ends_at: dto.trial_ends_at.map(short).filter(|s| !s.is_empty()),
            period_end: dto.period_end.map(short).filter(|s| !s.is_empty()),
        }
    }
}
```

Add `pub account: Option<AccountInfo>,` to `Session` and `account: None` in `Session::new`. Export from `lib.rs`: `pub use session::{AccountInfo, Entitlement, Session};` (keep whatever is exported today).

In `client.rs`: `Pulled` gains `pub account: Option<AccountInfo>` (the `Debug` impl prints `.field("account", &self.account)`); `login` sets it:

```rust
        let mut session = Session::new(dto.token, dto.expires_at, account_id, dto.vault_id);
        session.account = dto.account.map(AccountInfo::from);
        Ok(session)
```

and `pull` fills `account: dto.account.map(AccountInfo::from)`.

In `error_from`, after the 410 branch:

```rust
    if response.status == 402 {
        let said = serde_json::from_slice::<Body>(&response.body).ok();
        if said.is_some_and(|b| b.error.code == "account_frozen") {
            return SyncError::AccountFrozen;
        }
    }
```

In `error.rs`: add the variant after `AccountDeleted`:

```rust
    /// The account's trial ended or payment lapsed: the server refuses
    /// writes and new devices until it is paid or set by the operator.
    AccountFrozen,
```

with `code()` → `"account_frozen"` and Display → `"this account is frozen: the trial ended or payment lapsed"`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p havenkeys-sync-client`
Expected: PASS. `cargo build --workspace` now fails in `havenkeys-client/src/error.rs` (the exhaustive `From<SyncError>`); that is Task 3's first step. To keep the workspace compiling at this commit, add the arm now:

```rust
            SyncError::AccountFrozen => Self::fixed(
                "account_frozen",
                "This account is frozen: the trial ended or payment lapsed. The vault is read-only.",
            ),
```

and `(SyncError::AccountFrozen, "account_frozen")` to the cases list in that file's test.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-sync-client crates/havenkeys-client/src/error.rs
git commit -m "feat(sync-client): read the account's plan on login and sync; 402 is AccountFrozen"
```

---

### Task 2: Core store — the plan columns (schema 7)

**Files:**
- Modify: `crates/havenkeys-core/src/store.rs`
- Modify: `crates/havenkeys-core/src/vault.rs` (three one-line methods)
- Modify: `crates/havenkeys-core/src/lib.rs` or `model.rs` only if `PlanRecord` must be re-exported (export from `store`)
- Test: `crates/havenkeys-core/src/store.rs` unit tests

**Interfaces:**
- Produces:
  - `havenkeys_core::store::Entitlement { Full, Frozen }` (`Copy, Clone, Debug, PartialEq, Eq, Serialize` with `rename_all = "lowercase"`).
  - `havenkeys_core::store::PlanRecord { pub status: Option<String>, pub entitlement: Entitlement, pub trial_ends_at: Option<String>, pub period_end: Option<String> }` (`Clone, Debug, PartialEq, Eq, Default` where default is `status: None, entitlement: Full`).
  - `Store::plan(&self) -> Result<PlanRecord>` (Full defaults when the row is absent), `Store::set_plan(&mut self, &PlanRecord) -> Result<()>`.
  - `VaultService::plan()`, `VaultService::set_plan()`, `VaultService::entitlement() -> Result<Entitlement>`; all safe while locked.

- [ ] **Step 1: Write the failing tests**

In `crates/havenkeys-core/src/store.rs` tests:

```rust
    #[test]
    fn plan_defaults_to_full_and_round_trips() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(store.plan().unwrap(), PlanRecord::default());
        store.set_account(&sample_account()).unwrap();
        assert_eq!(store.plan().unwrap().entitlement, Entitlement::Full);
        let plan = PlanRecord {
            status: Some("trialing".into()),
            entitlement: Entitlement::Frozen,
            trial_ends_at: Some("2026-10-09T00:00:00Z".into()),
            period_end: None,
        };
        store.set_plan(&plan).unwrap();
        assert_eq!(store.plan().unwrap(), plan);
        // set_account keeps the plan (like the cursor).
        store.set_account(&sample_account()).unwrap();
        assert_eq!(store.plan().unwrap(), plan);
    }

    #[test]
    fn a_schema_6_vault_gains_the_plan_columns() {
        let conn = Connection::open_in_memory().unwrap();
        let schema6 = SCHEMA
            .replace("    plan_status    TEXT,\n", "")
            .replace("    entitlement    TEXT    NOT NULL DEFAULT 'full',\n", "")
            .replace("    trial_ends_at  TEXT,\n", "")
            .replace("    period_end     TEXT,\n", "");
        assert_ne!(schema6, SCHEMA, "the replace must have removed the new columns");
        conn.execute_batch(&schema6).unwrap();
        conn.pragma_update(None, "user_version", 6).unwrap();
        let store = Store::init(conn).unwrap();
        assert_eq!(store.plan().unwrap(), PlanRecord::default());
        let v: i64 = store.conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }
```

`sample_account()` is this `AccountRecord` literal, added to the test module if it has no equivalent helper: `AccountRecord { account_id: Uuid::from_u128(1), email: "user@example.com".into(), server_url: "https://vault.example.com".into(), server_cursor: 0, max_header_rev: 0, last_synced_at: None }`. Also update `a_schema_5_vault_gains_the_local_blob_table` so its schema-5 DDL strips the four plan columns too, and assert `plan()` works after the 5 → 7 path.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p havenkeys-core store::`
Expected: compile errors.

- [ ] **Step 3: The schema and migration**

In `store.rs`: `pub const SCHEMA_VERSION: i64 = 7;`. In `SCHEMA`'s `account` table, after `last_synced_at INTEGER`:

```sql
    plan_status    TEXT,
    entitlement    TEXT    NOT NULL DEFAULT 'full',
    trial_ends_at  TEXT,
    period_end     TEXT,
```

(keep the trailing comma discipline of the file; the last column must not end with a comma). Add:

```rust
/// Schema 6 → 7: the account's plan as the server last reported it (spec
/// 2026-10-07 §6.2). Plaintext: it is not a secret and the lock screen
/// shows it.
const MIGRATE_6_TO_7: &str = "
ALTER TABLE account ADD COLUMN plan_status TEXT;
ALTER TABLE account ADD COLUMN entitlement TEXT NOT NULL DEFAULT 'full';
ALTER TABLE account ADD COLUMN trial_ends_at TEXT;
ALTER TABLE account ADD COLUMN period_end TEXT;
";
```

and in `init()` replace the `5` arm and add a `6` arm:

```rust
            5 => {
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(MIGRATE_5_TO_6)?;
                tx.execute_batch(MIGRATE_6_TO_7)?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
            6 => {
                let tx = conn.unchecked_transaction()?;
                tx.execute_batch(MIGRATE_6_TO_7)?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
```

- [ ] **Step 4: The record and the accessors**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Entitlement {
    Full,
    Frozen,
}

impl Entitlement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Frozen => "frozen",
        }
    }
    fn parse(s: &str) -> Self {
        if s == "frozen" { Self::Frozen } else { Self::Full }
    }
}

/// The plan as the server last reported it. Absent (a vault that never
/// synced against a server with plans) means Full.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanRecord {
    pub status: Option<String>,
    pub entitlement: Entitlement,
    pub trial_ends_at: Option<String>,
    pub period_end: Option<String>,
}

impl Default for PlanRecord {
    fn default() -> Self {
        Self { status: None, entitlement: Entitlement::Full, trial_ends_at: None, period_end: None }
    }
}

impl Store {
    pub fn plan(&self) -> Result<PlanRecord> {
        self.conn
            .query_row(
                "SELECT plan_status, entitlement, trial_ends_at, period_end FROM account WHERE id = 1",
                [],
                |r| {
                    Ok(PlanRecord {
                        status: r.get::<_, Option<String>>(0)?,
                        entitlement: Entitlement::parse(&r.get::<_, String>(1)?),
                        trial_ends_at: r.get(2)?,
                        period_end: r.get(3)?,
                    })
                },
            )
            .optional()
            .map(|p| p.unwrap_or_default())
            .map_err(Into::into)
    }

    /// Only the plan columns; the rest of the row is untouched. A vault
    /// with no account row has no plan to record.
    pub fn set_plan(&mut self, plan: &PlanRecord) -> Result<()> {
        self.conn.execute(
            "UPDATE account SET plan_status = ?1, entitlement = ?2, trial_ends_at = ?3, period_end = ?4
              WHERE id = 1",
            params![plan.status, plan.entitlement.as_str(), plan.trial_ends_at, plan.period_end],
        )?;
        Ok(())
    }
}
```

`set_account`'s `ON CONFLICT` clause already leaves the new columns alone. Put the `impl Store` block beside `set_cursor`.

In `vault.rs`, next to `account()`:

```rust
    /// The account's plan as last synced. Safe while locked.
    pub fn plan(&self) -> Result<PlanRecord> {
        self.store.plan()
    }

    pub fn set_plan(&mut self, plan: &PlanRecord) -> Result<()> {
        self.store.set_plan(plan)
    }

    /// Full unless the server said otherwise. Safe while locked.
    pub fn entitlement(&self) -> Result<Entitlement> {
        Ok(self.store.plan()?.entitlement)
    }
```

with `use crate::store::{Entitlement, PlanRecord};` (or the path the file uses for `AccountRecord`).

- [ ] **Step 5: Run the tests**

Run: `cargo test -p havenkeys-core`
Expected: PASS, including `schema_version_is_set` (now 7) and the two migration tests.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-core
git commit -m "feat(core): store the account's plan and entitlement (schema 7)"
```

---

### Task 3: Client core — persist the plan, gate the writes, the invite preview

**Files:**
- Modify: `crates/havenkeys-client/src/events.rs` (`plan_changed` with a default body)
- Modify: `crates/havenkeys-client/src/sync.rs` (`connect`, `sync_now`, `failed`, `push`, `push_batches`, new `record_plan`)
- Modify: `crates/havenkeys-client/src/client.rs` (`require_full`)
- Modify: `crates/havenkeys-client/src/account.rs` (`AccountStatus` fields, `change_master_password`, `preview_invite`)
- Modify: `crates/havenkeys-client/src/error.rs` (`account_frozen()` constructor beside `open_website()`)
- Test: `crates/havenkeys-client/tests/round_trip.rs` (new tests), `crates/havenkeys-client/src/account.rs` unit test for `preview_invite`

**Interfaces:**
- Consumes: `Session.account`, `Pulled.account`, `SyncError::AccountFrozen` (Task 1); `VaultService::{plan, set_plan, entitlement}`, `PlanRecord`, `Entitlement` (Task 2).
- Produces:
  - `ClientEvents::plan_changed(&self) {}` (default no-op; desktop and mobile override later).
  - `HavenClient::require_full(&self) -> ClientResult<()>` → `ClientError::account_frozen()` (code `account_frozen`).
  - `AccountStatus` gains `plan_status: Option<String>`, `entitlement: Entitlement` (serialized `"full" | "frozen"`), `trial_ends_at: Option<String>`, `period_end: Option<String>`.
  - `HavenClient::preview_invite(&self, invite: &str) -> ClientResult<InvitePreview>` with `#[derive(Serialize)] pub struct InvitePreview { pub email: String, pub server_url: String }`.
  - `HavenClient::record_plan(&self, info: Option<&AccountInfo>) -> ClientResult<()>` (pub(crate)).

- [ ] **Step 1: Write the failing tests**

In `crates/havenkeys-client/tests/round_trip.rs` (use its existing two-device fixture and the server's `billing` module as the sync-client test does):

```rust
#[tokio::test]
async fn the_plan_is_stored_on_login_and_sync_and_a_402_freezes_at_once() {
    let fx = fixture().await;                       // the file's signed-in client + server
    let status = fx.client.account_status().unwrap().unwrap();
    assert_eq!(status.entitlement, havenkeys_core::store::Entitlement::Full);
    assert_eq!(status.plan_status.as_deref(), Some("complimentary"));

    // The server freezes the account behind the client's back; the next
    // write is refused with the client's own code and the stored plan flips.
    freeze(&fx).await;                               // billing::set_status(.., Frozen, ..)
    let err = fx.client.push(fx.staged_login()).await.unwrap_err();
    assert_eq!(err.code, "account_frozen");
    assert_eq!(fx.client.account_status().unwrap().unwrap().entitlement, havenkeys_core::store::Entitlement::Frozen);
    assert_eq!(fx.events.plan_changes(), 1);

    // require_full now refuses before any request.
    assert_eq!(fx.client.require_full().unwrap_err().code, "account_frozen");
    assert_eq!(fx.client.push(fx.staged_login()).await.unwrap_err().code, "account_frozen");
}

#[tokio::test]
async fn a_sync_that_says_full_unfreezes() {
    let fx = fixture().await;
    freeze(&fx).await;
    fx.client.sync_now().await.unwrap();
    assert_eq!(fx.client.account_status().unwrap().unwrap().entitlement, havenkeys_core::store::Entitlement::Frozen);
    unfreeze(&fx).await;                             // billing::set_status(.., Complimentary, ..)
    fx.client.sync_now().await.unwrap();
    assert_eq!(fx.client.account_status().unwrap().unwrap().entitlement, havenkeys_core::store::Entitlement::Full);
    fx.client.push(fx.staged_login()).await.unwrap();
}

#[tokio::test]
async fn a_frozen_account_still_syncs_reads_and_can_delete_itself() {
    let fx = fixture().await;
    fx.client.push(fx.staged_login()).await.unwrap();
    freeze(&fx).await;
    let report = fx.client.sync_now().await.unwrap();
    let _ = report; // read sync works
    assert!(fx.client.vault().unwrap().list_items().unwrap().len() >= 1);
    // change_master_password is refused before any derivation
    let err = fx.client.change_master_password(fx.password(), fx.password()).await.unwrap_err();
    assert_eq!(err.code, "account_frozen");
}
```

`fixture()`, `freeze()`, `unfreeze()`, `fx.staged_login()`, `fx.password()` and `fx.events.plan_changes()` are helpers to add to that file in the style of its existing ones: `events` is the file's `ClientEvents` test implementation (add an `AtomicUsize` it bumps in `plan_changed`). The `events` test implementations at `client.rs:347`, `tests/pairing.rs:110` and `tests/round_trip.rs:132` need no change because `plan_changed` has a default body.

In `crates/havenkeys-client/src/account.rs` tests:

```rust
    #[test]
    fn preview_invite_decodes_without_touching_the_network() {
        // Build the string the server would: "HKINV1-" + base64url(nopad) of
        // {"server":"https://vault.example.com","email":"me@example.com","account":"<uuid>","secret":"s"}
        let json = br#"{"server":"https://vault.example.com","email":"Me@Example.com","account":"00000000-0000-0000-0000-000000000001","secret":"AAAAAAAAAAAAAAAAAAAAAA"}"#;
        let raw = format!("HKINV1-{}", data_encoding::BASE64URL_NOPAD.encode(json));
        let client = test_client();                  // the module's existing offline test client
        let preview = client.preview_invite(&raw).unwrap();
        assert_eq!(preview.email, "me@example.com");
        assert_eq!(preview.server_url, "https://vault.example.com");
        assert_eq!(client.preview_invite("HKINV1-nope").unwrap_err().code, "invalid_input");
    }
```

(`data-encoding` is a dependency of the sync client; add it to `havenkeys-client`'s `[dev-dependencies]` if the client crate does not already have it.)

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p havenkeys-client`
Expected: compile errors.

- [ ] **Step 3: Events and errors**

`events.rs`, at the end of the trait:

```rust
    /// The stored plan changed (a sync or a login said so, or a write was
    /// refused as frozen). Shells re-read `account_status`.
    fn plan_changed(&self) {}
```

`error.rs`, beside `open_website()`:

```rust
    pub fn account_frozen() -> Self {
        Self::fixed(
            "account_frozen",
            "This account is frozen: the trial ended or payment lapsed. The vault is read-only.",
        )
    }
```

and make the `From<SyncError>` arm from Task 1 call `Self::account_frozen()`.

- [ ] **Step 4: Persist and gate**

`client.rs`, beside `require_online`:

```rust
    /// Refuse a write while the account is frozen (spec 2026-10-07 §6.3).
    /// Decided from the stored entitlement, so it answers offline too.
    pub fn require_full(&self) -> ClientResult<()> {
        match self.vault()?.entitlement()? {
            havenkeys_core::store::Entitlement::Full => Ok(()),
            havenkeys_core::store::Entitlement::Frozen => Err(ClientError::account_frozen()),
        }
    }
```

`sync.rs`:

```rust
    /// Store what the server said about the plan. `None` (a server from
    /// before plans) changes nothing. Announces a change to the shells.
    pub(crate) fn record_plan(&self, info: Option<&havenkeys_sync_client::AccountInfo>) -> ClientResult<()> {
        let Some(info) = info else { return Ok(()) };
        let plan = havenkeys_core::store::PlanRecord {
            status: Some(info.status.clone()),
            entitlement: match info.entitlement {
                havenkeys_sync_client::Entitlement::Full => havenkeys_core::store::Entitlement::Full,
                havenkeys_sync_client::Entitlement::Frozen => havenkeys_core::store::Entitlement::Frozen,
            },
            trial_ends_at: info.trial_ends_at.clone(),
            period_end: info.period_end.clone(),
        };
        let changed = {
            let mut vault = self.vault()?;
            let before = vault.plan()?;
            if before != plan {
                vault.set_plan(&plan)?;
            }
            before != plan
        };
        if changed {
            self.events.plan_changed();
        }
        Ok(())
    }

    /// A refused write: the server's word is final until the next sync says
    /// otherwise, so the stored entitlement flips now.
    fn mark_frozen(&self) {
        let flipped = self.vault().ok().and_then(|mut v| {
            let mut plan = v.plan().ok()?;
            if plan.entitlement == havenkeys_core::store::Entitlement::Frozen {
                return None;
            }
            plan.entitlement = havenkeys_core::store::Entitlement::Frozen;
            v.set_plan(&plan).ok()
        });
        if flipped.is_some() {
            self.events.plan_changed();
        }
    }
```

In `failed()`, add the arm `SyncError::AccountFrozen => self.mark_frozen(),` before `_ => {}`.

In `connect()`, right after `self.set_online(session);` inside the block (the session is moved into `set_online`, so take the account first):

```rust
        let plan = session.account.clone();
        {
            let vault = self.vault()?;
            if !vault.is_unlocked() || vault.epoch() != epoch {
                return Err(havenkeys_core::Error::Locked.into());
            }
            self.set_online(session);
        }
        let _ = self.record_plan(plan.as_ref());
```

In `sync_now()`, inside the pull loop right after `let next = pulled.cursor;`: `let plan = pulled.account.clone();` and after `apply_remote_changes` succeeds: `let _ = self.record_plan(plan.as_ref());`.

In `push()` and `push_batches()`, first line of the body: `self.require_full()?;`.

In `account.rs`, `change_master_password`: add `self.require_full()?;` right after `self.require_online()?;`. Extend `AccountStatus`:

```rust
    pub plan_status: Option<String>,
    pub entitlement: havenkeys_core::store::Entitlement,
    pub trial_ends_at: Option<String>,
    pub period_end: Option<String>,
```

and in `account_status()` read `let plan = self.vault()?.plan()?;` and fill them. Add:

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitePreview {
    pub email: String,
    pub server_url: String,
}

impl HavenClient {
    /// What an invite says, before anything is typed or sent: the account's
    /// email and the server it points at. Pure decoding, no network.
    pub fn preview_invite(&self, invite: &str) -> ClientResult<InvitePreview> {
        let invite = invite_parser::decode(invite.trim())?;
        let email = NormalizedEmail::parse(&invite.email)?;
        Ok(InvitePreview {
            email: email.as_str().to_string(),
            server_url: invite.server,
        })
    }
}
```

(`NormalizedEmail::as_str` or whatever accessor the type has; the decode error already converts to `invalid_input`, which the test pins.)

- [ ] **Step 5: Run the tests**

Run: `cargo test -p havenkeys-client` (Postgres running) and `cargo test --workspace --no-run`
Expected: client tests PASS; the workspace compiles. The desktop and mobile `ClientEvents` impls compile unchanged.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-client
git commit -m "feat(client): store the plan from login and sync, refuse writes while frozen, preview an invite"
```

---

### Task 4: Protocol and bridge — `frozen`, the entitlement on `status` and `find_matches`, the gate

**Files:**
- Modify: `crates/havenkeys-protocol/src/message.rs` (`ErrorCode::Frozen`, `message()`, `ResultBody::Status`, `ResultBody::FindMatches`)
- Modify: `crates/havenkeys-protocol/tests/messages.rs` (the fuzz/round-trip tests that enumerate codes or build `Status`)
- Modify: `packages/protocol/src/index.ts` (`ERROR_CODES`, `Entitlement`, status and find_matches result types and parsers) + its tests
- Modify: `crates/havenkeys-bridge/src/dispatch.rs`
- Modify: `crates/havenkeys-bridge/tests/bridge.rs`
- Modify: `crates/havenkeys-native-host/tests/host.rs` only if it builds a `Status` result by hand
- Modify: `apps/extension/src/i18n/en.ts`, `pt-BR.ts` (`errors.bridge.frozen`)
- Test: bridge tests, protocol tests, `pnpm --filter @havenkeys/protocol test`, `pnpm --filter @havenkeys/extension test -- bridge-parity`

**Interfaces:**
- Produces:
  - `ErrorCode::Frozen` (wire `frozen`), message verbatim `Your HavenKeys trial has ended. The vault is read-only until you subscribe.`
  - `ResultBody::Status { state, vault_exists, entitlement: Entitlement }` and `ResultBody::FindMatches { matches, entitlement: Entitlement }` with `pub enum Entitlement { Full, Frozen }` (`rename_all = "snake_case"`, wire `full`/`frozen`).
  - TS: `export type Entitlement = "full" | "frozen"`; `{ type: "status"; state; vaultExists; entitlement }`; `{ type: "find_matches"; matches; entitlement }`; `"frozen"` in `ERROR_CODES`.
  - The bridge refuses while frozen: `fill_item` (after the origin check), `check_login`, `save_login`, `check_passkey_create`, `passkey_create`, `start_sso`, `check_sso`, `save_sso`, `find_identity`, `fill_identity`, `find_cards`, `fill_card`, `save_card`. Answers while frozen: `status`, `lock`, `show_unlock`, `find_matches`, `get_totp`, `find_passkeys`, `passkey_get`, `passkey_status`, `open_item`, `open_identity`, `generator_options`, `generate_password`.

- [ ] **Step 1: Write the failing bridge tests**

Append to `crates/havenkeys-bridge/tests/bridge.rs`:

```rust
fn freeze(f: &Fixture) {
    let mut v = f.vault.lock().unwrap();
    let mut plan = v.plan().unwrap();
    plan.status = Some("trialing".into());
    plan.entitlement = havenkeys_core::store::Entitlement::Frozen;
    v.set_plan(&plan).unwrap();
}

/// Frozen (spec 2026-10-07 §6.3): the menu's lookups and every fill, save
/// and passkey creation are refused; status, TOTP and passkey sign-in stay.
#[test]
fn a_frozen_account_refuses_autofill_and_keeps_reading() {
    let f = online_fixture();
    freeze(&f);
    let status = call(&f, serde_json::json!({"type": "status"}));
    assert_eq!(status["result"]["state"], "unlocked");
    assert_eq!(status["result"]["entitlement"], "frozen");
    let found = find(&f, "https://github.com/");
    assert_eq!(found["result"]["entitlement"], "frozen");
    assert_eq!(found["result"]["matches"].as_array().unwrap().len(), 1, "metadata still lists the site's logins");
    assert_eq!(error_code(&fill(&f, f.github, "https://github.com/")), Some("frozen"));
    assert!(totp(&f, f.github, "https://github.com/")["result"]["code"].is_string());
    for req in [
        serde_json::json!({"type": "check_login", "url": "https://github.com/", "username": "octo", "password": "pw"}),
        serde_json::json!({"type": "save_login", "url": "https://github.com/", "username": "octo", "password": "pw"}),
        serde_json::json!({"type": "find_identity", "url": "https://github.com/"}),
        serde_json::json!({"type": "find_cards", "url": "https://shop.example/"}),
        serde_json::json!({"type": "start_sso", "itemId": f.typeform, "url": "https://typeform.com/"}),
        serde_json::json!({"type": "check_passkey_create", "url": "https://github.com/", "rpId": "github.com", "userName": "octo", "excludeCredentials": [], "conditional": false}),
    ] {
        assert_eq!(error_code(&call(&f, req.clone())), Some("frozen"), "{req}");
    }
    assert_eq!(f.changes.load(Ordering::SeqCst), 0, "nothing was written");
    // Open in desktop still works for a matched item.
    let opened = call(&f, serde_json::json!({"type": "open_item", "itemId": f.github, "url": "https://github.com/"}));
    assert!(opened["result"].is_object(), "{opened}");
}

/// A1 holds when frozen: the wrong origin is denied, not frozen.
#[test]
fn a1_wrong_origin_is_still_denied_when_frozen() {
    let f = fixture();
    freeze(&f);
    assert_eq!(error_code(&fill(&f, f.github, "https://evil.com/")), Some("denied"));
    assert_eq!(error_code(&totp(&f, f.github, "https://evil.com/")), Some("denied"));
}
```

Match the exact request field sets to the `Request` enum (`deny_unknown_fields`): check `message.rs` for `check_login`, `start_sso`, `check_passkey_create` fields and adjust the JSON. Also assert `status["result"]["entitlement"] == "full"` in `legitimate_flow_works`.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p havenkeys-bridge`
Expected: FAIL (no `entitlement` field; `fill_item` succeeds while frozen).

- [ ] **Step 3: Protocol, Rust**

In `message.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Entitlement {
    Full,
    Frozen,
}
```

`ResultBody::Status { state: LockState, vault_exists: bool, entitlement: Entitlement }`; `ResultBody::FindMatches { matches: Vec<Match>, entitlement: Entitlement }` (keep the existing field order with the new one last). Add `Frozen,` to `ErrorCode` after `Offline`, and in `message()`:

```rust
            ErrorCode::Frozen => "Your HavenKeys trial has ended. The vault is read-only until you subscribe.",
```

(one line, a plain string literal: the parity test's regex needs it). Fix every constructor of these two variants: `dispatch.rs` (`Status`, `FindMatches`), `havenkeys-protocol/tests/messages.rs`, `havenkeys-native-host/tests/host.rs` if any, and the extension/desktop fakes found by `grep -rn '"find_matches"\|type: "status"' apps/extension/src packages/protocol/src --include=*.ts` (test fakes that return `{ type: "status", state, vaultExists }` must gain `entitlement: "full"`, because the TS parser is exact-keys).

- [ ] **Step 4: Protocol, TypeScript**

`packages/protocol/src/index.ts`: `export type Entitlement = "full" | "frozen";` and `const ENTITLEMENTS: readonly Entitlement[] = ["full", "frozen"];`. Result types:

```ts
  | { type: "status"; state: LockState; vaultExists: boolean; entitlement: Entitlement }
  | { type: "find_matches"; matches: Match[]; entitlement: Entitlement }
```

Parsers: `hasExactKeys(v, ["type", "state", "vaultExists", "entitlement"])` plus `ENTITLEMENTS.includes(v.entitlement as Entitlement)`; the same for `find_matches` (find its `case` and add the key and check). Add `"frozen"` to `ERROR_CODES` (before `"internal"`). Update the package's own tests that build these results. Add to `apps/extension/src/i18n/en.ts` `bridge`: `frozen: "Your HavenKeys trial has ended. The vault is read-only until you subscribe.",` and to `pt-BR.ts`: `frozen: "Seu período de teste do HavenKeys terminou. O cofre fica somente leitura até você assinar.",`.

- [ ] **Step 5: The gate in `dispatch`**

In `dispatch.rs`, after `require_enabled(v)?;`:

```rust
    // Frozen (spec 2026-10-07 §6.3): nothing that fills, saves or creates
    // a passkey, and none of the menus' lookups. Reading stays: status,
    // the popup's list, TOTP, passkey sign-in, opening in the desktop.
    // `fill_item` and `get_totp` check the origin first so a wrong origin
    // is still `denied` (attack 1).
    let frozen = v.entitlement().map_err(code)? == havenkeys_core::store::Entitlement::Frozen;
    if frozen && refused_when_frozen(req) {
        return Err(ErrorCode::Frozen);
    }
```

with

```rust
fn refused_when_frozen(req: &Request) -> bool {
    matches!(
        req,
        Request::CheckLogin { .. }
            | Request::SaveLogin { .. }
            | Request::CheckPasskeyCreate { .. }
            | Request::PasskeyCreate { .. }
            | Request::StartSso { .. }
            | Request::CheckSso { .. }
            | Request::SaveSso { .. }
            | Request::FindIdentity { .. }
            | Request::FillIdentity { .. }
            | Request::FindCards { .. }
            | Request::FillCard { .. }
            | Request::SaveCard { .. }
    )
}
```

and in the `FillItem` arm, before `fill_for_page`:

```rust
            // Origin first (attack 1), then the plan.
            let matched = v
                .find_matches(url, top_url.as_deref())
                .map_err(code)?
                .iter()
                .any(|s| s.id == *item_id);
            if !matched {
                return Err(ErrorCode::Denied);
            }
            if frozen {
                return Err(ErrorCode::Frozen);
            }
```

`Status` and `FindMatches` results carry `entitlement: if frozen { Entitlement::Frozen } else { Entitlement::Full }` (import `havenkeys_protocol::Entitlement`). `request_class` and `changes_items` are unchanged.

- [ ] **Step 6: Run everything that this touches**

Run: `cargo test -p havenkeys-protocol -p havenkeys-bridge -p havenkeys-native-host && pnpm --filter @havenkeys/protocol test && pnpm --filter @havenkeys/extension test`
Expected: all PASS, including `bridge-parity`. The extension suite will show which test fakes still build an old-shaped `status` result; fix them (add `entitlement: "full"`) in this task.

- [ ] **Step 7: Commit**

```bash
git add crates/havenkeys-protocol crates/havenkeys-bridge crates/havenkeys-native-host packages/protocol apps/extension
git commit -m "feat(bridge): frozen accounts refuse autofill; status and find_matches carry the entitlement"
```

---

### Task 5: Extension — the frozen popup, the menu stays out

**Files:**
- Modify: `apps/extension/src/messaging/popup.ts` (`PopupState`)
- Modify: `apps/extension/src/background/popup-handler.ts` (`state()`, `stateForError`, a `popup_open_pricing` request)
- Modify: `apps/extension/src/popup/popup.ts` (`render` frozen branch, a `frozenRow`)
- Modify: `apps/extension/src/background/inline-handler.ts` (`openMenu`)
- Modify: `apps/extension/src/i18n/en.ts`, `pt-BR.ts` (`popup.frozen.*`)
- Modify: `apps/extension/manifest/base.json` only if `chrome.tabs.create` needs a permission it lacks (`tabs` is not needed for `create` with a URL; check the manifest and leave it alone if so)
- Test: `apps/extension/src/background/popup-handler.test.ts`, `apps/extension/src/popup/popup.test.ts`, `apps/extension/src/background/inline-handler.test.ts`

**Interfaces:**
- Consumes: `status.entitlement`, `find_matches.entitlement`, `BridgeError` code `frozen` (Task 4).
- Produces: `PopupState { kind: "frozen"; site: string | null; matches: Match[] }`; `PopupRequest { type: "popup_open_pricing" }` which opens `https://havenkeys.net/pricing` in a new tab; strings `t.popup.frozen.{title, body, subscribe, code, open}`.

- [ ] **Step 1: Write the failing tests**

`popup-handler.test.ts`:

```ts
  it("reports frozen with the site's logins when the desktop says so", async () => {
    const c = fakeClient((r) =>
      r.type === "status"
        ? { type: "status", state: "unlocked", vaultExists: true, entitlement: "frozen" }
        : r.type === "find_matches"
          ? { type: "find_matches", matches: [ghMatch], entitlement: "frozen" }
          : (() => { throw new BridgeError("frozen", "x"); })(),
    );
    const h = createPopupHandler(c, async () => ({ id: 1, url: "https://github.com/login" }));
    const r = await h.handle({ type: "popup_state" });
    expect(r).toEqual({ ok: true, value: { kind: "frozen", site: "github.com", matches: [ghMatch] } });
    // The identity lookup is not even attempted.
    expect(c.seen.map((x) => x.type)).toEqual(["status", "find_matches"]);
  });

  it("a frozen fill is refused with the frozen message", async () => {
    const c = fakeClient((r) =>
      r.type === "find_matches"
        ? { type: "find_matches", matches: [ghMatch], entitlement: "frozen" }
        : (() => { throw new BridgeError("frozen", en.errors.bridge.frozen); })(),
    );
    const h = createPopupHandler(c, async () => ({ id: 1, url: "https://github.com/login" }));
    const r = await h.handle({ type: "popup_fill", itemId: ghMatch.id });
    expect(r).toEqual({ ok: false, message: en.errors.bridge.frozen });
  });

  it("popup_open_pricing opens the pricing page", async () => {
    const created: string[] = [];
    (globalThis as { chrome?: unknown }).chrome = { tabs: { create: async (o: { url: string }) => { created.push(o.url); return {}; } } };
    const h = createPopupHandler(fakeClient(() => unlocked), async () => undefined);
    expect(await h.handle({ type: "popup_open_pricing" })).toEqual({ ok: true, value: null });
    expect(created).toEqual(["https://havenkeys.net/pricing"]);
  });
```

(`ghMatch` and `unlocked` are the file's fixtures; `unlocked` gains `entitlement: "full"` in Task 4.)

`popup.test.ts`:

```ts
  it("frozen: shows the notice, Subscribe, and Code for a login with TOTP but no Fill", async () => {
    replies = {
      popup_state: async () => ({ ok: true, value: { kind: "frozen", site: "github.com", matches: [{ id: "11111111-1111-4111-8111-111111111111", title: "GitHub", username: "octo", hasTotp: true, signInWith: null }] } }),
    };
    vi.resetModules();
    await import("./popup");
    await vi.waitFor(() => expect(document.querySelector(".notice.frozen")).not.toBeNull());
    const texts = [...document.querySelectorAll("button")].map((b) => b.textContent);
    expect(texts).toContain(en.popup.frozen.subscribe);
    expect(texts).toContain(en.popup.frozen.code);
    expect(texts).not.toContain(en.popup.fill);
  });
```

(adjust the `Match` fixture to the real `Match` shape in `packages/protocol`; `en.popup.fill` is whatever key labels the Fill button today.)

`inline-handler.test.ts`:

```ts
  it("no menu when the account is frozen", async () => {
    const { h, requests } = setup((r) =>
      r.type === "find_matches" ? { type: "find_matches", matches: [ghMatch], entitlement: "frozen" } : { type: "status", state: "unlocked", vaultExists: true, entitlement: "frozen" },
    );
    expect(await h.handleContent(frame(), { type: "cs_open_menu", kind: "login" })).toEqual({ ok: false });
    expect(requests.map((r) => r.type)).toEqual(["find_matches"]);
  });
```

- [ ] **Step 2: Run to see them fail**

Run: `pnpm --filter @havenkeys/extension test`
Expected: the new tests FAIL.

- [ ] **Step 3: Implement**

`messaging/popup.ts`: add `| { kind: "frozen"; site: string | null; matches: Match[] }` to `PopupState` and `| { type: "popup_open_pricing" }` to `PopupRequest`; in `parsePopupRequest` add `case "popup_open_pricing":` to the one-key group.

`popup-handler.ts`, `state()`: after the `status.state !== "unlocked"` check, when `status.entitlement === "frozen"`: get the URL as today; if none, `return { kind: "frozen", site: null, matches: [] }`; else `const found = await client.request({ type: "find_matches", url }); return { kind: "frozen", site: displayHost(url), matches: found.matches };` (no identity lookup). In `stateForError`, add `case "frozen": return { kind: "frozen", site: null, matches: [] };`. In `handle()`, add:

```ts
      case "popup_open_pricing":
        await chrome.tabs.create({ url: PRICING_URL });
        return { ok: true, value: null };
```

with `const PRICING_URL = "https://havenkeys.net/pricing";` at the top. `fillFromPopup` needs no change: the bridge's `frozen` error flows through `fail(e)` as the fixed message.

`popup/popup.ts`, `render()`: add

```ts
    case "frozen": {
      setPill(t.popup.pill.frozen, "frozen");
      const box = notice(t.popup.frozen.title, t.popup.frozen.body);
      box.classList.add("frozen");
      const subscribe = h("button", { className: "btn", text: t.popup.frozen.subscribe });
      subscribe.type = "button";
      subscribe.addEventListener("click", () => void send({ type: "popup_open_pricing" }));
      box.append(subscribe);
      const parts: Node[] = [box];
      if (state.site) parts.push(truncates(h("div", { className: "site", text: state.site })));
      if (state.matches.length > 0) parts.push(h("ul", { className: "list" }, ...state.matches.map(frozenRow)));
      main.replaceChildren(...parts);
      return;
    }
```

and `frozenRow(m)`: the same `li.item` as `matchRow` with the avatar (`popup_open_item`), the title and `userLine`, and, only `if (m.hasTotp)`, the Code button (`popup_totp` → shows the code; the code button then fills via `popup_fill_totp`), but no Fill button. Factor the shared part out of `matchRow` so the two rows do not duplicate the DOM building. Add `.pill.frozen` to `popup.css` (brass tone like `locked`) and `.notice.frozen`.

`inline-handler.ts`, `openMenu`, after the `find_matches` try/catch:

```ts
    if (found.entitlement === "frozen") return { ok: false };
```

(keep `items = found.matches`; restructure the `try` so `found` is in scope.)

Strings, `en.ts`:

```ts
    frozen: {
      title: "Your trial has ended",
      body: "The vault is read-only until you subscribe. You can still show a one-time code here, and open items in the HavenKeys app.",
      subscribe: "Subscribe",
      code: "Code",
    },
```

and `pill.frozen: "Read-only"`; `pt-BR.ts`: `title: "Seu período de teste terminou"`, `body: "O cofre fica somente leitura até você assinar. Você ainda pode ver um código de uso único aqui e abrir itens no app HavenKeys."`, `subscribe: "Assinar"`, `code: "Código"`, `pill.frozen: "Somente leitura"`.

- [ ] **Step 4: Run the suite and package**

Run: `pnpm --filter @havenkeys/extension test && pnpm --filter @havenkeys/extension typecheck && pnpm package:extension`
Expected: PASS; the zip builds (the native host sidecar is rebuilt by the package script; check the script's output names the host binary).

- [ ] **Step 5: Commit**

```bash
git add apps/extension
git commit -m "feat(extension): frozen popup with TOTP and Subscribe; the inline menu stays out"
```

---

### Task 6: Desktop — commands, events, onboarding, sidebar, read-only

**Files:**
- Modify: `apps/desktop/src-tauri/src/account.rs` (`preview_invite`, `open_signup`, `open_pricing`)
- Modify: `apps/desktop/src-tauri/src/events.rs` (`plan_changed` → `PLAN_CHANGED_EVENT`)
- Modify: `apps/desktop/src-tauri/src/commands.rs` (`stage_write` gains `require_full`), `import.rs` (`import_file`, `restore_backup`), `sync.rs` (`bridge_error`: `"account_frozen" => ErrorCode::Frozen`)
- Modify: `apps/desktop/src-tauri/build.rs`, `capabilities/main.json`, `src/lib.rs`
- Modify: `apps/desktop/src/lib/api.ts`, `lib/types.ts`
- Modify: `apps/desktop/src/App.tsx`, `views/WelcomeScreen.tsx`, `views/VaultScreen.tsx`
- Modify: `apps/desktop/src/i18n/en.ts`, `pt-BR.ts`
- Test: `apps/desktop/src/views/VaultScreen.test.tsx`, new `apps/desktop/src/views/WelcomeScreen.test.tsx`, `apps/desktop/src/App.test.tsx` if present; `cargo test -p havenkeys-desktop`

**Interfaces:**
- Consumes: `HavenClient::{preview_invite, require_full}`, `AccountStatus` fields, `ClientEvents::plan_changed` (Task 3); `ErrorCode::Frozen` (Task 4).
- Produces: Tauri commands `preview_invite(invite) -> InvitePreview`, `open_signup()`, `open_pricing()`; event `plan_changed`; TS `api.previewInvite`, `api.openSignup`, `api.openPricing`, `api.onPlanChanged`; `AccountStatus` TS gains `planStatus`, `entitlement`, `trialEndsAt`, `periodEnd`; `VaultScreen` prop `frozen: boolean`; strings `welcome.createAccount`, `welcome.tabInvite` → "I have a setup code", `welcome.invite` → "Setup code", `welcome.inviteHint`, `welcome.creatingFor(email, server)`, `vault.trialDays(n)`, `vault.frozenTitle`, `vault.frozenBody`, `vault.subscribe`, `errors.codes.account_frozen`.

- [ ] **Step 1: Rust commands and events**

`account.rs`:

```rust
#[tauri::command]
pub fn preview_invite(state: State<'_, AppState>, invite: String) -> CmdResult<havenkeys_client::InvitePreview> {
    state.client().preview_invite(&invite)
}

pub const SIGNUP_URL: &str = "https://havenkeys.net/signup";
pub const PRICING_URL: &str = "https://havenkeys.net/pricing";

/// Fixed addresses only: the renderer cannot hand the OS a URL.
#[tauri::command]
pub fn open_signup() -> CmdResult<()> {
    tauri_plugin_opener::open_url(SIGNUP_URL, None::<&str>).map_err(|_| CmdError::open_website())
}

#[tauri::command]
pub fn open_pricing() -> CmdResult<()> {
    tauri_plugin_opener::open_url(PRICING_URL, None::<&str>).map_err(|_| CmdError::open_website())
}
```

Register all three in `build.rs` `COMMANDS` (`"preview_invite"`, `"open_signup"`, `"open_pricing"`), `capabilities/main.json` (`allow-preview-invite`, `allow-open-signup`, `allow-open-pricing`) and `lib.rs` `generate_handler!`. `events.rs`: `pub const PLAN_CHANGED_EVENT: &str = "plan_changed";` and `fn plan_changed(&self) { let _ = self.app.emit(PLAN_CHANGED_EVENT, ()); }`. `commands.rs` `stage_write`: after `state.require_online()?;` add `state.require_full()?;` where `AppState::require_full` forwards to `self.client.require_full()` (add it beside `require_online` in `state.rs`); `empty_trash` and `resync_vault`: `empty_trash` gets `require_full`, `resync_vault` does not (it reads). `import.rs`: `import_file` and `restore_backup` get `state.require_full()?;` after `require_online`. `sync.rs` `bridge_error`: add `"account_frozen" => ErrorCode::Frozen,`.

Unit test in `account.rs` (or the crate's tests): `SIGNUP_URL` and `PRICING_URL` start with `https://havenkeys.net/`.

- [ ] **Step 2: TypeScript API**

`types.ts`: `AccountStatus` gains `planStatus: string | null; entitlement: "full" | "frozen"; trialEndsAt: string | null; periodEnd: string | null;` and `export interface InvitePreview { email: string; serverUrl: string }`. `api.ts`: `previewInvite: (invite: string) => call<InvitePreview>("preview_invite", { invite: invite.trim() })`, `openSignup: () => call<void>("open_signup")`, `openPricing: () => call<void>("open_pricing")`, `onPlanChanged: (cb: () => void) => listen("plan_changed", cb)` in the file's event-subscription style (see `onSignedOut`).

- [ ] **Step 3: Welcome screen**

`WelcomeScreen.tsx`: above the segmented tabs add a primary **Create account** button calling `api.openSignup()` with the note `t.welcome.createAccountNote` ("Opens havenkeys.net in your browser. You come back here with a setup code."). Rename the first tab's copy through i18n (`tabInvite: "I have a setup code"`, `subInvite: "Set up this computer with the code from your email."`, `invite: "Setup code"`, `inviteHint: "One line, from your sign-up email or from whoever runs your server. It works once."`). In `InvitePanel`, add `const [preview, setPreview] = useState<InvitePreview | null>(null);` and an effect on `invite` (debounced 300 ms) that calls `api.previewInvite(invite)` when the trimmed value starts with `HKINV1-`, sets the preview, and clears it on error; render `preview && <p className="wf-hint">{t.welcome.creatingFor(preview.email, preview.serverUrl)}</p>` under the textarea ("Creating an account for {email} on {server}."). Write `WelcomeScreen.test.tsx` in the style of `DeleteAccountSection.test.tsx`: mocks `api.previewInvite` to resolve `{ email: "me@example.com", serverUrl: "https://api.havenkeys.net" }`, types `HKINV1-abc` into the textarea, waits, and asserts the sentence appears; asserts the Create account button calls `openSignup`.

- [ ] **Step 4: Plan in the shell**

`App.tsx`: add `const [account, setAccount] = useState<AccountStatus | null>(null);` loaded in the same effect as `deviceStatus` (`api.accountStatus().then(setAccount, () => setAccount(null))`) and re-loaded on `api.onPlanChanged`; `const frozen = account?.entitlement === "frozen";`. Pass `readOnly={!device?.online || frozen}` and `frozen={frozen}` to `VaultScreen`. Render, above the offline banner, when frozen:

```tsx
      {frozen && (
        <div className="banner banner-warn" role="status">
          <span><strong>{t.vault.frozenTitle}</strong> {t.vault.frozenBody}</span>
          <button className="btn btn-quiet" type="button" onClick={() => void api.openPricing()}>
            {t.vault.subscribe}
          </button>
        </div>
      )}
```

`VaultScreen.tsx`: new prop `frozen: boolean`; the sidebar foot's `.conn` line shows `t.vault.trialDays(n)` when `account?.planStatus === "trialing"` and `account.trialEndsAt` parses (`n = Math.max(0, Math.ceil((Date.parse(trialEndsAt) - Date.now()) / 86_400_000))`), `t.vault.readOnlyFrozen` when frozen, else the existing offline/connected text; re-load `account` on `api.onPlanChanged` (subscribe in the mount effect and unsubscribe on unmount). Export and delete account stay enabled: `SettingsView` keeps `online={!readOnly}` for import and gets `frozen={frozen}` so `ImportSection` is disabled with `t.vault.frozenImport` while `ExportSection` and `DeleteAccountSection` keep `online={!device?.online}`.

Strings: `en.ts` `vault.trialDays: (n: number) => n === 1 ? "Trial: 1 day left" : \`Trial: ${n} days left\``, `vault.readOnlyFrozen: "Read-only (trial ended)"`, `vault.frozenTitle: "Your trial has ended."`, `vault.frozenBody: "The vault is read-only. You can still open, copy, export and sign in with passkeys."`, `vault.subscribe: "Subscribe"`, `vault.frozenImport: "Importing needs an active plan."`, `errors.codes.account_frozen: "This account is frozen: the trial ended or payment lapsed. The vault is read-only."` (Rust's message word for word), `welcome.createAccount: "Create account"`, `welcome.createAccountNote`, `welcome.creatingFor: (email, server) => \`Creating an account for ${email} on ${server}.\``; pt-BR counterparts (`"Teste: faltam N dias"`, `"Somente leitura (teste encerrado)"`, `"Seu período de teste terminou."`, `"O cofre fica somente leitura. Você ainda pode abrir, copiar, exportar e entrar com passkeys."`, `"Assinar"`, `"Importar precisa de um plano ativo."`, `"Esta conta está congelada: o teste terminou ou o pagamento falhou. O cofre fica somente leitura."`, `"Criar conta"`, `"Abre havenkeys.net no navegador. Você volta aqui com um código de configuração."`, `"Criando uma conta para ${email} em ${server}."`).

Tests: `VaultScreen.test.tsx`: with `accountStatus` resolving `{ ..., planStatus: "trialing", entitlement: "full", trialEndsAt: <now + 5 days> }` the foot shows "Trial: 5 days left"; with `entitlement: "frozen"` and `frozen` prop true the New button is disabled and the foot says read-only. `App` test if the file has one: the frozen banner appears with the Subscribe button.

- [ ] **Step 5: Run**

Run: `cargo test -p havenkeys-desktop && pnpm --filter @havenkeys/desktop test && pnpm --filter @havenkeys/desktop typecheck`
Expected: PASS. Then `pnpm --filter @havenkeys/desktop tauri dev` once to click through: welcome shows Create account; paste a setup code (from the stage 1 server run locally, or any `HKINV1-` string built as in Task 3's test) and see the "Creating an account for…" line.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop
git commit -m "feat(desktop): create account and setup code on welcome, trial and frozen states in the shell"
```

---

### Task 7: Android — status, autofill, settings, onboarding

**Files:**
- Modify: `crates/havenkeys-mobile/src/vault.rs` (`Status` fields, `frozen()`), `src/onboarding.rs` (`preview_invite`), `src/events.rs` (`PlanChanged`)
- Regenerate the UniFFI bindings per `docs/android.md` / the owner's `android-build-env` memory (JAVA_HOME, ANDROID_NDK_HOME)
- Modify: `apps/android/app/src/main/kotlin/net/havenkeys/android/data/{AccountRepository,AutofillRepository,VaultRepository}.kt`, `autofill/HavenAutofillService.kt`, `ui/onboarding/{OnboardingScreen,OnboardingViewModel}.kt`, `ui/settings/{SettingsScreen,SettingsViewModel}.kt`, `ui/components/ErrorText.kt`, the events hub (`VaultEvent.PlanChanged`)
- Modify: `res/values/strings.xml`, `res/values-pt-rBR/strings.xml`
- Modify: `app/src/test/.../fakes/FakeRepositories.kt` (positional `status()` helper; fake `frozen()`, `previewInvite()`)
- Test: `OnboardingViewModelTest`, `SettingsViewModelTest`, a new `HavenAutofillServiceTest` or `FillPlannerTest` case, `PasskeyCreateViewModelTest`

**Interfaces:**
- Produces: UniFFI `Status` gains `plan_status: Option<String>`, `entitlement: String` (`"full"`/`"frozen"`), `trial_ends_at: Option<String>`; `MobileVault::frozen(&self) -> bool` (safe while locked, false on any error); `MobileVault::preview_invite(&self, invite: String) -> MobileResult<InvitePreview>` with `#[derive(uniffi::Record)] pub struct InvitePreview { pub email: String, pub server_url: String }`; `VaultEvents::plan_changed()` on the foreign trait; Kotlin `AutofillRepository.frozen(): Boolean`, `AccountRepository.previewInvite(invite): Outcome<InvitePreview>`, `VaultEvent.PlanChanged`; strings `error_account_frozen`, `onboarding_create_account`, `onboarding_create_account_sub`, `onboarding_setup_code_choice`, `onboarding_setup_code_sub`, `onboarding_setup_code`, `onboarding_setup_code_hint`, `onboarding_creating_for`, `settings_plan`, `settings_plan_trial` (quantity string), `settings_plan_frozen`, `settings_subscribe`, `autofill_save_frozen`, `autofill_card_save_frozen`.

- [ ] **Step 1: Rust mobile**

`vault.rs` `Status`: add the three fields; `status()` fills them from `account_status()` (`plan_status`, `entitlement.as_str().to_string()`, `trial_ends_at`). Add:

```rust
    /// Safe while locked. Any failure reads as not frozen: a damaged store
    /// must not turn Autofill off for good.
    pub fn frozen(&self) -> bool {
        self.client
            .vault()
            .and_then(|v| Ok(v.entitlement()?))
            .map(|e| e == havenkeys_core::store::Entitlement::Frozen)
            .unwrap_or(false)
    }
```

`onboarding.rs`: `pub fn preview_invite(&self, invite: String) -> MobileResult<InvitePreview>` mapping `client.preview_invite(&invite)`. `events.rs`: `Event::PlanChanged`, `fn plan_changed(&self) { self.pump.send(Event::PlanChanged); }`, `Event::PlanChanged => app.plan_changed()`, and `fn plan_changed(&self);` on the `VaultEvents` foreign trait. The mobile `send`/`send_returning` need no change (`client.push` refuses). `credentials.rs` `passkey_create`: add `self.client.require_full()?;` after `require_online` so the error names the cause before any work. Mobile tests: the `Status` constructions in `src/vault.rs:245` and `tests/*` gain the fields; `tests/round_trip.rs` gets a frozen case mirroring the client's (`autofill_save` → `account_frozen`, `frozen()` true after a sync).

Regenerate the bindings (the command is in `docs/android.md`; the owner's memory note has the env vars).

- [ ] **Step 2: Kotlin data layer and events**

`VaultEventsHub`: add `PlanChanged` to `VaultEvent` and implement `planChanged()` by emitting it. `AutofillRepository`: `suspend fun frozen(): Boolean` → `(rust { vault.frozen() } as? Outcome.Ok)?.value ?: false`. `AccountRepository`: `suspend fun previewInvite(invite: String): Outcome<InvitePreview>` → `rust { vault.previewInvite(invite.trim()) }`. `ErrorText.kt`: `"account_frozen" to R.string.error_account_frozen`. Fakes: `FakeAutofillRepository.frozen` var, `FakeAccountRepository.previewInvite` returning a settable outcome, the positional `status()` helper gains `null, "full", null` (or named args).

- [ ] **Step 3: Autofill**

`HavenAutofillService.respond`: after the `routed == null` check, `if (repo.frozen()) return null` (no datasets, no save prompt). `saveMessage`: add `"account_frozen" -> if (card) R.string.autofill_card_save_frozen else R.string.autofill_save_frozen`. Test: a `FillPlanner`/service-level unit test that a frozen repository yields a null response (follow the existing autofill tests' fixture).

- [ ] **Step 4: Settings and onboarding**

`SettingsUiState` gains `planStatus: String?`, `entitlement: String`, `trialEndsAt: String?`; set from `vault.status()` in `init` and re-read on `VaultEvent.PlanChanged`. `AccountGroup` gains a row after "Signed in as": title `settings_plan`, detail `settings_plan_trial` (plural quantity, days left computed from `trialEndsAt` with `java.time.Instant.parse`) when `planStatus == "trialing"`, `settings_plan_frozen` when `entitlement == "frozen"`, else the plan status word; when frozen, a `settings_subscribe` row that opens `https://havenkeys.net/pricing` with the `openLink` helper from `HealthScreen.kt` (move it to `ui/components/OpenLink.kt`). Editors already surface `account_frozen` through `ErrorText`.

`ChooseStep`: a first choice `onboarding_create_account` / `onboarding_create_account_sub` with `HavenIcon.Globe` that opens `https://havenkeys.net/signup`; the invite choice becomes `onboarding_setup_code_choice` / `onboarding_setup_code_sub`. `InviteStep`: `SecretRow(invite, R.string.onboarding_setup_code, …, hint = onboarding_setup_code_hint)`; the ViewModel gains `previewInvite(invite)` (debounced in the composable via `LaunchedEffect(invite.text)`) that sets `state.invitePreview: InvitePreview?`; when set, render the same two-row `InsetGroup` as `KitSummary` under the field with `onboarding_creating_for` as the section header.

Strings (EN; PT-BR mirrored): `error_account_frozen` = "This account is frozen: the trial ended or payment lapsed. The vault is read-only."; `onboarding_create_account` = "Create account"; `onboarding_create_account_sub` = "Opens havenkeys.net. You come back here with a setup code."; `onboarding_setup_code_choice` = "I have a setup code"; `onboarding_setup_code_sub` = "Set up this phone with the code from your email."; `onboarding_setup_code` = "Setup code"; `onboarding_setup_code_hint` = "One line, from your sign-up email or from whoever runs your server. It works once."; `onboarding_creating_for` = "Creating an account for"; `settings_plan` = "Plan"; `settings_plan_trial` plurals one "Trial: 1 day left" / other "Trial: %d days left"; `settings_plan_frozen` = "Trial ended. Read-only."; `settings_subscribe` = "Subscribe"; `autofill_save_frozen` = "Not saved: the trial ended and the vault is read-only."; `autofill_card_save_frozen` likewise for the card.

Tests: `OnboardingViewModelTest.previewShowsTheEmailAndServer`; `SettingsViewModelTest.showsTheTrialAndTheFrozenState`; autofill null response when frozen; `PasskeyCreateViewModelTest`: a `Failed("account_frozen")` surfaces as the error.

- [ ] **Step 5: Run**

Run: `cargo test -p havenkeys-mobile`, then the Android unit tests with the env from the memory note: `cd apps/android && ./gradlew :app:testDebugUnitTest`. Build a debug APK (`scripts/build-android.sh` or the gradle task `docs/android.md` names) to confirm the bindings compile.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-mobile apps/android
git commit -m "feat(android): plan status, frozen autofill and saving, create account and setup code in onboarding"
```

---

### Task 8: Docs and the index

**Files:**
- Modify: `docs/native-messaging.md` (the `entitlement` field on `status` and `find_matches`, the `frozen` code, the refused list)
- Modify: `docs/security-model.md` §25 (the client-side list of what stops and what stays; the honest limitation is already there)
- Modify: `docs/android.md` (plan row, frozen autofill)
- Modify: `docs/security-review.md` (an entry: frozen is enforced by open-source clients; `find_matches` still lists metadata while frozen, by design, so the popup can offer TOTP)
- Modify: `docs/superpowers/plans/2026-10-09-self-signup-index.md` (stage 3 planned/done, decisions recorded)

- [ ] **Step 1: Write the docs** as the file structure lists; keep every number and list identical to the code (grep `refused_when_frozen` for the refused list).
- [ ] **Step 2: Commit**

```bash
git add docs
git commit -m "docs: frozen accounts in the native messaging, security model and Android docs"
```

---

## Self-review notes

- Spec §6.1 (Create account, I have a setup code, the email/server preview) → Tasks 3, 6, 7.
- Spec §6.2 (store the account object, trial line, frozen banner with Subscribe) → Tasks 1, 2, 3, 6, 7; the Subscribe URL is `/pricing` by ruling.
- Spec §6.3 table → desktop (Task 6: editors, import, change password stop; export and delete stay), extension (Tasks 4, 5: popup limited to TOTP and Open in desktop by ruling; menu, Fill, sign-in, save prompts, generator through the menu, cards and identity stop), Android (Task 7: autofill datasets and saving stop), passkeys (Task 4: `passkey_get` and `find_passkeys` stay, `check_passkey_create` and `passkey_create` refused; Task 7 mobile `passkey_create` refused).
- Spec §8 client bullets: frozen refuses `fill_item`, the menu (through `find_matches.entitlement` and the bridge's refusal of the menu's other lookups), passkey creation; allows the popup's TOTP and passkey sign-in; wrong origin still `denied` → Task 4 tests.
- Review Focus items 1 to 5 → Tasks 3, 1, 2, 3, 7 as noted inline.
- Deviation from the index's refusal list: `find_matches` is answered while frozen (metadata only, origin-bound) because the popup needs the list to offer TOTP; the inline menu honours `entitlement` from that answer and the bridge refuses everything that would fill or save. The index's `generate_password` remains answered (it touches no item); the generator's insert path only exists through the menu, which does not open.
