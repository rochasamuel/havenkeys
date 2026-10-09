# Self-Service Signup — Stage 1: Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `havenkeys-server` gains plans with a computed entitlement (every existing account becomes `complimentary`), refuses writes and new devices with `402 account_frozen`, and, when `HAVENKEYS_SIGNUP=open`, lets anyone create an account by verifying their email with a code sent over SMTP and receiving an ordinary invite.

**Architecture:** One migration adds `subscriptions`, `billing_events`, `signup_codes` and four columns on `accounts`. A `billing` module owns the single `entitlement()` function and the single `set_status()` writer; the `Session` extractor computes the entitlement for every authenticated request and the four write points call `session.require_full()`. A `mail` module hides SMTP (`lettre`) behind a `Mailer` trait so tests use an in-memory recorder. Two unauthenticated routes, `signup/start` and `signup/verify`, reuse the invite, rate-limit and activation code that already exists. One daily task sweeps codes, abandoned signups and counters, and sends the two trial emails.

**Tech Stack:** Rust (axum 0.8, tokio-postgres, deadpool-postgres, lettre 0.11, hmac/sha2, clap 4), Postgres tests through `scripts/test-server.sh`.

**Spec:** `docs/superpowers/specs/2026-10-07-self-signup-and-plans-design.md` (§4, §5, §8 server bullets, §3). Index: `docs/superpowers/plans/2026-10-09-self-signup-index.md`.

## Global Constraints

- Trial length: 14 days from activation. Grace after `past_due`: 7 days. Signup invite validity: 24 h. Code validity: 15 min, 5 attempts. Abandoned signup deleted after 7 days.
- Rate limits: `signup-ip:<addr>` 5 `start` per hour; `signup-email:<hash>` 3 per hour; `verify` failures count against the same IP key, which refuses `verify` at 10 (a start plus a code's five attempts must fit; the spec's "same key" is kept, its threshold for `verify` is not the start threshold).
- `HAVENKEYS_SIGNUP` is `open` or `off`, default `off`; with `off` both signup routes answer `404`. `open` requires `SMTP_URL`, `SMTP_FROM` and `HAVENKEYS_PUBLIC_URL`.
- Error bodies keep the existing shape `{"error":{"code":..,"message":..}}`. New code: `account_frozen`, status 402. Messages are fixed strings that never quote a request value (CLAUDE.md §39).
- Logs carry `account_id` and outcomes only. Never an email, a code, an invite, a token (CLAUDE.md §40). `tests/no_logging.rs` is the gate.
- The code is stored only as `HMAC-SHA-256(SERVER_SECRET, code)`; the invite secret only as SHA-256, as today.
- Shipped migrations are never edited: everything here is `0004_plans.sql`.
- Frozen never blocks: read sync, login from a known device, `GET /v1/vault/header`, device list and revoke, logout, `POST /v1/account/delete`.
- Every email exists in `en` and `pt-BR`, plain text, no tracking, no HTML.
- No commit carries a `Co-Authored-By` trailer (user preference).
- Postgres-backed tests need `scripts/test-server.sh` running (default URL `postgres://postgres:postgres@localhost:5433/postgres`). Run the server tests with `cargo test -p havenkeys-server`.

## Review Focus

1. **The same email typed two ways** (`User@Example.com` at `start`, `user@example.com` at `verify`) must verify: both routes normalize with `email::normalize`. (Task 5: `verify_accepts_the_email_in_any_spelling`.)
2. **Resend**: a second `start` while a code is live must make the old code dead and the new one work, with attempts reset. (Task 5: `a_second_start_replaces_the_code`.)
3. **SMTP down at `verify`**: the invite is created and returned (the page shows it), the mail failure is only logged; SMTP down at `start` answers `503` for a new and an existing email alike. (Task 5: `verify_still_returns_the_invite_when_mail_fails`, `start_answers_503_uniformly_when_mail_fails`.)
4. **A frozen account's known device whose session expired** must still log in (`200`), and only a device the account has never used gets `402`. (Task 2: `a_frozen_account_is_refused_every_write_and_new_device`.)
5. **An admin-invited address used in the signup form** must not hit the unique-email error: its invite is replaced and the old one stops working. (Task 5: `an_admin_invited_account_gets_its_invite_replaced_by_signup`.)

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/havenkeys-server/migrations/0004_plans.sql` (new) | `subscriptions`, `billing_events`, `signup_codes`; `accounts.created_by`, `terms_version`, `terms_accepted_at`, `locale`; complimentary rows for every existing account |
| `crates/havenkeys-server/src/db.rs` | registers `0004_plans` |
| `crates/havenkeys-server/src/billing/mod.rs` (new) | `Status`, `Subscription`, `Entitlement`, `entitlement()`, `load()`, `from_row()`, `set_status()`, `note()`, `account_json()` |
| `crates/havenkeys-server/src/billing/provider.rs` (new) | `ProviderEvent`, `apply_provider_event()`: the gateway seam |
| `crates/havenkeys-server/src/billing/jobs.rs` (new) | `run_daily()`: sweeps and trial emails |
| `crates/havenkeys-server/src/error.rs` | `ApiError::AccountFrozen` → 402 |
| `crates/havenkeys-server/src/auth/mod.rs` | `Session.subscription`, `Session.entitlement`, `require_full()` |
| `crates/havenkeys-server/src/auth/rate_limit.rs` | `charge_window()`, `over_window()` |
| `crates/havenkeys-server/src/routes/{items,account,pairings,auth,sync}.rs` | `require_full()` at the write points; new-device refusal; `account` object |
| `crates/havenkeys-server/src/routes/accounts.rs` | activation starts the trial |
| `crates/havenkeys-server/src/routes/signup.rs` (new) | `start`, `verify` |
| `crates/havenkeys-server/src/routes/mod.rs` | `AppState.mailer`, `AppState.signup_url`, the two routes |
| `crates/havenkeys-server/src/mail/mod.rs` (new) | `Mail`, `Mailer`, `Smtp`, `Recording` |
| `crates/havenkeys-server/src/mail/templates.rs` (new) | `Locale`, the five emails in `en` and `pt-BR` |
| `crates/havenkeys-server/src/config.rs` | `signup_open`, `public_url`, `smtp`; `check_public_url()` |
| `crates/havenkeys-server/src/admin.rs` | `new-account --trial`, `set-plan`, plan column in `list-accounts` |
| `crates/havenkeys-server/src/erase.rs` | deletes the account's `signup_codes` row |
| `crates/havenkeys-server/src/main.rs` | builds the mailer; daily task runs `run_daily` |
| `crates/havenkeys-server/tests/support/mod.rs` | `start_with`, `start_signup`, signup helpers, `login_with_device` |
| `crates/havenkeys-server/tests/{plans,signup,jobs}.rs` (new), `tests/{admin,schema,no_logging,deletion}.rs` | tests |
| `docs/deployment.md`, `docs/self-hosting.md`, `docs/server-sync.md`, `docs/security-model.md`, `CLAUDE.md` | documentation |

---

### Task 1: Migration and the billing module

**Files:**
- Create: `crates/havenkeys-server/migrations/0004_plans.sql`
- Create: `crates/havenkeys-server/src/billing/mod.rs`
- Modify: `crates/havenkeys-server/src/db.rs:24-34` (the `MIGRATIONS` list)
- Modify: `crates/havenkeys-server/src/lib.rs:15-28` (`pub mod billing;`)
- Modify: `crates/havenkeys-server/tests/schema.rs:78-91` (`migrations_are_idempotent`: 3 → 4)
- Test: `crates/havenkeys-server/src/billing/mod.rs` (unit), `crates/havenkeys-server/tests/schema.rs`

**Interfaces:**
- Produces:
  - `billing::Status { Trialing, Active, PastDue, Frozen, Complimentary }` with `as_str()` / `parse()`.
  - `billing::Subscription { status, trial_ends_at, current_period_end, grace_ends_at }`.
  - `billing::Entitlement { Full, Frozen }`; `billing::entitlement(&Subscription, DateTime<Utc>) -> Entitlement`; `billing::entitlement_of(Option<&Subscription>, now) -> Entitlement` (missing row → `Full`).
  - `billing::load(db, account_id) -> Result<Option<Subscription>, tokio_postgres::Error>`; `billing::from_row(&Row, first_column) -> Option<Subscription>`.
  - `billing::Actor { Admin, System, Provider }`; `billing::set_status(db, account_id, actor, status, until, reason)`; `billing::note(db, account_id, reason)`.
  - `billing::account_json(Option<&Subscription>, now) -> serde_json::Value`.
  - Constants `TRIAL_DAYS = 14`, `GRACE_DAYS = 7`, `PLAN_PERSONAL = "personal"`.

- [ ] **Step 1: Write the migration**

`crates/havenkeys-server/migrations/0004_plans.sql`:

```sql
-- Plans, trials and self-service signup
-- (docs/superpowers/specs/2026-10-07-self-signup-and-plans-design.md §4.3, §5.1, §5.2).

-- One row per account. The stored status is what an operator or a payment
-- provider last set; whether the account may write is computed from the row
-- and the clock (billing::entitlement), never flipped by a scheduled job.
CREATE TABLE subscriptions (
  account_id         UUID PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
  plan               TEXT NOT NULL,
  status             TEXT NOT NULL CHECK (status IN
                       ('trialing', 'active', 'past_due', 'frozen', 'complimentary')),
  trial_ends_at      TIMESTAMPTZ,                  -- set at activation: now() + 14 days
  current_period_end TIMESTAMPTZ,                  -- paid until
  grace_ends_at      TIMESTAMPTZ,                  -- past_due: 7 days of grace
  provider           TEXT,                         -- NULL | 'stripe' | 'mercadopago'
  provider_customer  TEXT,
  provider_ref       TEXT,                         -- the gateway's subscription id
  updated_at         TIMESTAMPTZ NOT NULL
);

-- Append-only audit trail of status changes and notices. No card data ever
-- reaches the server. Erased with the account (ON DELETE CASCADE).
CREATE TABLE billing_events (
  id          BIGSERIAL PRIMARY KEY,
  account_id  UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  old_status  TEXT,
  new_status  TEXT NOT NULL,
  actor       TEXT NOT NULL CHECK (actor IN ('admin', 'system', 'provider')),
  reason      TEXT NOT NULL,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX billing_events_by_account ON billing_events (account_id, created_at);

-- Every account that exists today keeps what it has: a complimentary plan
-- with no end date (§5.5).
INSERT INTO subscriptions (account_id, plan, status, updated_at)
SELECT id, 'personal', 'complimentary', now() FROM accounts;

-- Self-service signup (§4.3). The code is stored keyed by the server secret:
-- six digits alone would fall to an offline brute force of a dumped table.
-- The locale and terms version travel with the code so `verify`, which only
-- carries email and code, can record them on the account.
CREATE TABLE signup_codes (
  email_normalized TEXT PRIMARY KEY,
  code_hash        BYTEA NOT NULL CHECK (octet_length(code_hash) = 32),
  locale           TEXT NOT NULL,
  terms_version    TEXT NOT NULL,
  expires_at       TIMESTAMPTZ NOT NULL,
  attempts         INTEGER NOT NULL DEFAULT 0,
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX signup_codes_by_expiry ON signup_codes (expires_at);

-- Who created the account (the abandoned-signup sweep only touches
-- 'signup'), the LGPD consent record, and the language for account notices.
ALTER TABLE accounts
  ADD COLUMN created_by        TEXT NOT NULL DEFAULT 'admin'
                               CHECK (created_by IN ('admin', 'signup')),
  ADD COLUMN terms_version     TEXT,
  ADD COLUMN terms_accepted_at TIMESTAMPTZ,
  ADD COLUMN locale            TEXT;
```

- [ ] **Step 2: Register it and fix the idempotency count**

In `crates/havenkeys-server/src/db.rs`, append to `MIGRATIONS`:

```rust
    ("0004_plans", include_str!("../migrations/0004_plans.sql")),
```

In `crates/havenkeys-server/tests/schema.rs`, `migrations_are_idempotent`: change `assert_eq!(applied, 3);` to `assert_eq!(applied, 4);`.

- [ ] **Step 3: Write the failing unit tests for `entitlement`**

Create `crates/havenkeys-server/src/billing/mod.rs` with only the test module first (the items it names do not exist yet):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, day, 12, 0, 0).unwrap()
    }

    fn sub(status: Status) -> Subscription {
        Subscription {
            status,
            trial_ends_at: None,
            current_period_end: None,
            grace_ends_at: None,
        }
    }

    #[test]
    fn complimentary_is_always_full() {
        assert_eq!(entitlement(&sub(Status::Complimentary), at(1)), Entitlement::Full);
    }

    #[test]
    fn active_is_full_until_its_period_ends() {
        let mut s = sub(Status::Active);
        assert_eq!(entitlement(&s, at(1)), Entitlement::Full, "no period end: paid");
        s.current_period_end = Some(at(10));
        assert_eq!(entitlement(&s, at(9)), Entitlement::Full);
        assert_eq!(entitlement(&s, at(10)), Entitlement::Frozen);
    }

    #[test]
    fn trialing_is_full_until_the_trial_ends_and_before_activation() {
        let mut s = sub(Status::Trialing);
        assert_eq!(entitlement(&s, at(1)), Entitlement::Full, "not activated yet");
        s.trial_ends_at = Some(at(15));
        assert_eq!(entitlement(&s, at(14)), Entitlement::Full);
        assert_eq!(entitlement(&s, at(15)), Entitlement::Frozen);
    }

    #[test]
    fn past_due_is_full_only_during_grace() {
        let mut s = sub(Status::PastDue);
        assert_eq!(entitlement(&s, at(1)), Entitlement::Frozen, "no grace recorded");
        s.grace_ends_at = Some(at(8));
        assert_eq!(entitlement(&s, at(7)), Entitlement::Full);
        assert_eq!(entitlement(&s, at(8)), Entitlement::Frozen);
    }

    #[test]
    fn frozen_is_frozen() {
        assert_eq!(entitlement(&sub(Status::Frozen), at(1)), Entitlement::Frozen);
    }

    #[test]
    fn a_missing_row_is_full() {
        assert_eq!(entitlement_of(None, at(1)), Entitlement::Full);
    }

    #[test]
    fn status_round_trips_through_its_name() {
        for s in [
            Status::Trialing,
            Status::Active,
            Status::PastDue,
            Status::Frozen,
            Status::Complimentary,
        ] {
            assert_eq!(Status::parse(s.as_str()), Some(s));
        }
        assert_eq!(Status::parse("whatever"), None);
    }

    #[test]
    fn account_json_names_the_entitlement_and_dates() {
        let mut s = sub(Status::Trialing);
        s.trial_ends_at = Some(at(15));
        let v = account_json(Some(&s), at(1));
        assert_eq!(v["status"], "trialing");
        assert_eq!(v["entitlement"], "full");
        assert_eq!(v["trialEndsAt"], at(15).to_rfc3339());
        assert!(v["periodEnd"].is_null());
        let v = account_json(Some(&s), at(20));
        assert_eq!(v["entitlement"], "frozen");
        let v = account_json(None, at(1));
        assert_eq!(v["status"], "complimentary");
        assert_eq!(v["entitlement"], "full");
    }
}
```

Add `pub mod billing;` to `crates/havenkeys-server/src/lib.rs` (alphabetically, after `pub mod b64;`).

- [ ] **Step 4: Run the unit tests to see them fail**

Run: `cargo test -p havenkeys-server --lib billing`
Expected: compile error, `Status`, `entitlement` etc. not found.

- [ ] **Step 5: Write the module**

Above the test module in `crates/havenkeys-server/src/billing/mod.rs`:

```rust
//! Plans and entitlement (spec 2026-10-07 §5).
//!
//! `entitlement` is the only place that decides whether an account may write.
//! It is computed from the stored row and a clock, never flipped by a job
//! that has to fire on time; and `set_status` is the only writer of the
//! stored row, so every change leaves a `billing_events` line.

use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use uuid::Uuid;

pub mod jobs;
pub mod provider;

pub const PLAN_PERSONAL: &str = "personal";
pub const TRIAL_DAYS: i32 = 14;
pub const GRACE_DAYS: i32 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum Status {
    Trialing,
    Active,
    PastDue,
    Frozen,
    Complimentary,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trialing => "trialing",
            Self::Active => "active",
            Self::PastDue => "past_due",
            Self::Frozen => "frozen",
            Self::Complimentary => "complimentary",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "trialing" => Self::Trialing,
            "active" => Self::Active,
            "past_due" => Self::PastDue,
            "frozen" => Self::Frozen,
            "complimentary" => Self::Complimentary,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    pub status: Status,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub grace_ends_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

/// The one decision (spec §5.3). A date that has arrived counts as past.
pub fn entitlement(sub: &Subscription, now: DateTime<Utc>) -> Entitlement {
    let ended = |at: Option<DateTime<Utc>>| at.is_some_and(|t| t <= now);
    match sub.status {
        Status::Complimentary => Entitlement::Full,
        Status::Active if ended(sub.current_period_end) => Entitlement::Frozen,
        Status::Active => Entitlement::Full,
        Status::Trialing if ended(sub.trial_ends_at) => Entitlement::Frozen,
        Status::Trialing => Entitlement::Full,
        // No grace recorded means no grace: an operator who wants the
        // account open sets the date.
        Status::PastDue if sub.grace_ends_at.is_some() && !ended(sub.grace_ends_at) => {
            Entitlement::Full
        }
        Status::PastDue => Entitlement::Frozen,
        Status::Frozen => Entitlement::Frozen,
    }
}

/// An account without a row is one from before plans existed, which the
/// migration made complimentary; a row that is missing anyway is a bug,
/// and a bug must not hold a vault hostage.
pub fn entitlement_of(sub: Option<&Subscription>, now: DateTime<Utc>) -> Entitlement {
    sub.map_or(Entitlement::Full, |s| entitlement(s, now))
}

/// `status, trial_ends_at, current_period_end, grace_ends_at` read from a
/// row starting at column `first`; `None` when the status column is NULL
/// (a LEFT JOIN that found no row).
pub fn from_row(row: &tokio_postgres::Row, first: usize) -> Option<Subscription> {
    let status: Option<String> = row.get(first);
    let status = Status::parse(&status?)?;
    Some(Subscription {
        status,
        trial_ends_at: row.get(first + 1),
        current_period_end: row.get(first + 2),
        grace_ends_at: row.get(first + 3),
    })
}

pub async fn load(
    db: &impl GenericClient,
    account_id: Uuid,
) -> Result<Option<Subscription>, tokio_postgres::Error> {
    Ok(db
        .query_opt(
            "SELECT status, trial_ends_at, current_period_end, grace_ends_at
               FROM subscriptions WHERE account_id = $1",
            &[&account_id],
        )
        .await?
        .and_then(|row| from_row(&row, 0)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    Admin,
    System,
    Provider,
}

impl Actor {
    fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::System => "system",
            Self::Provider => "provider",
        }
    }
}

/// Set an account's plan status and record the change. `until` lands in
/// the column the status reads: `trial_ends_at` for trialing,
/// `current_period_end` for active, `grace_ends_at` for past_due; it is
/// ignored for frozen and complimentary. Provider columns are untouched.
pub async fn set_status(
    db: &impl GenericClient,
    account_id: Uuid,
    actor: Actor,
    status: Status,
    until: Option<DateTime<Utc>>,
    reason: &str,
) -> Result<(), tokio_postgres::Error> {
    let old: Option<String> = db
        .query_opt(
            "SELECT status FROM subscriptions WHERE account_id = $1 FOR UPDATE",
            &[&account_id],
        )
        .await?
        .map(|r| r.get(0));
    let (trial, period, grace) = match status {
        Status::Trialing => (until, None, None),
        Status::Active => (None, until, None),
        Status::PastDue => (None, None, until),
        Status::Frozen | Status::Complimentary => (None, None, None),
    };
    db.execute(
        "INSERT INTO subscriptions
           (account_id, plan, status, trial_ends_at, current_period_end, grace_ends_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, now())
         ON CONFLICT (account_id) DO UPDATE SET
           status = EXCLUDED.status,
           trial_ends_at = EXCLUDED.trial_ends_at,
           current_period_end = EXCLUDED.current_period_end,
           grace_ends_at = EXCLUDED.grace_ends_at,
           updated_at = now()",
        &[
            &account_id,
            &PLAN_PERSONAL,
            &status.as_str(),
            &trial,
            &period,
            &grace,
        ],
    )
    .await?;
    db.execute(
        "INSERT INTO billing_events (account_id, old_status, new_status, actor, reason)
         VALUES ($1, $2, $3, $4, $5)",
        &[&account_id, &old, &status.as_str(), &actor.as_str(), &reason],
    )
    .await?;
    Ok(())
}

/// An audit line that changes nothing (old and new status equal, actor
/// `system`). The trial emails use it to send once per account.
pub async fn note(
    db: &impl GenericClient,
    account_id: Uuid,
    reason: &str,
) -> Result<(), tokio_postgres::Error> {
    db.execute(
        "INSERT INTO billing_events (account_id, old_status, new_status, actor, reason)
         SELECT account_id, status, status, 'system', $2
           FROM subscriptions WHERE account_id = $1",
        &[&account_id, &reason],
    )
    .await?;
    Ok(())
}

/// What the apps are told on login and sync (spec §5.4).
pub fn account_json(sub: Option<&Subscription>, now: DateTime<Utc>) -> serde_json::Value {
    let status = sub.map_or(Status::Complimentary, |s| s.status);
    serde_json::json!({
        "status": status.as_str(),
        "entitlement": entitlement_of(sub, now).as_str(),
        "trialEndsAt": sub.and_then(|s| s.trial_ends_at).map(|t| t.to_rfc3339()),
        "periodEnd": sub.and_then(|s| s.current_period_end).map(|t| t.to_rfc3339()),
    })
}
```

Create empty placeholders so the module compiles: `crates/havenkeys-server/src/billing/jobs.rs` containing only `//! The daily task (Task 6).` and `crates/havenkeys-server/src/billing/provider.rs` containing only `//! The payment-gateway seam (Task 3).`

- [ ] **Step 6: Run the unit tests and the schema tests**

Run: `cargo test -p havenkeys-server --lib billing && cargo test -p havenkeys-server --test schema`
Expected: all PASS (8 unit tests; `migrations_are_idempotent` now sees 4).

- [ ] **Step 7: Commit**

```bash
git add crates/havenkeys-server/migrations/0004_plans.sql crates/havenkeys-server/src/billing crates/havenkeys-server/src/db.rs crates/havenkeys-server/src/lib.rs crates/havenkeys-server/tests/schema.rs
git commit -m "feat(server): plans schema and the entitlement function"
```

---

### Task 2: Entitlement on every request, `402 account_frozen`, the `account` object

**Files:**
- Modify: `crates/havenkeys-server/src/error.rs:11-30, 34-79`
- Modify: `crates/havenkeys-server/src/auth/mod.rs:152-218`
- Modify: `crates/havenkeys-server/src/routes/items.rs:65-70` (`write`)
- Modify: `crates/havenkeys-server/src/routes/account.rs:38-46` (`change_credentials`)
- Modify: `crates/havenkeys-server/src/routes/pairings.rs:168-174` (`approve`)
- Modify: `crates/havenkeys-server/src/routes/auth.rs:113-187` (`login`)
- Modify: `crates/havenkeys-server/src/routes/sync.rs:133-137` (`pull` response)
- Modify: `crates/havenkeys-server/tests/support/mod.rs` (`login_with_device`)
- Test: `crates/havenkeys-server/tests/plans.rs` (new)

**Interfaces:**
- Consumes: `billing::{from_row, load, entitlement_of, account_json, set_status, Actor, Status, Entitlement, Subscription}` from Task 1.
- Produces: `ApiError::AccountFrozen`; `Session { subscription: Option<Subscription>, entitlement: Entitlement }` and `Session::require_full(&self) -> Result<(), ApiError>`; `"account"` in the bodies of `POST /v1/auth/login` and `GET /v1/sync`; `support::login_with_device(server, &Account, name, device_id) -> (u16, Value)`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/havenkeys-server/tests/support/mod.rs`, after `login`:

```rust
/// A login with a device id the test chooses, and the raw answer.
pub async fn login_with_device(
    server: &TestServer,
    account: &Account,
    device_name: &str,
    device_id: Uuid,
) -> (u16, Value) {
    let res = server
        .post("/v1/auth/login")
        .json(&json!({
            "email": account.email,
            "authKey": data_encoding::BASE64.encode(&account.auth_key),
            "deviceId": device_id,
            "deviceName": device_name,
        }))
        .send()
        .await
        .unwrap();
    let status = res.status().as_u16();
    let text = res.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

/// Set the account's plan status directly, as `admin set-plan` would.
pub async fn set_plan(server: &TestServer, account_id: Uuid, status: havenkeys_server::billing::Status) {
    let db = server.db().await;
    havenkeys_server::billing::set_status(
        &db,
        account_id,
        havenkeys_server::billing::Actor::Admin,
        status,
        None,
        "test",
    )
    .await
    .unwrap();
}
```

Create `crates/havenkeys-server/tests/plans.rs`:

```rust
mod support;

use data_encoding::{BASE64, BASE64URL_NOPAD};
use havenkeys_server::billing::Status;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use support::*;
use uuid::Uuid;

#[tokio::test]
async fn a_frozen_account_is_refused_every_write_and_new_device() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    let (item, rev) = create_item(&server, &sess, b"o", b"d").await;
    set_plan(&server, account.account_id, Status::Frozen).await;

    // Writes: 402, nothing applied.
    let (status, body) = write(&server, &sess, vec![change(item, Some(rev), b"o2", b"d2")]).await;
    assert_eq!(status, 402);
    assert_eq!(body["error"]["code"], "account_frozen");
    let (status, _) = write(&server, &sess, vec![deletion(item, Some(rev))]).await;
    assert_eq!(status, 402);
    let res = server
        .post_as("/v1/account/credentials", &sess)
        .json(&credentials_body(&account.auth_key, &[1u8; 32], 0, b"h2"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 402);

    // A device the account never used: 402. A known device, even after its
    // session expired: 200.
    let (status, body) = login_with_device(&server, &account, "New laptop", Uuid::new_v4()).await;
    assert_eq!(status, 402, "{body}");
    assert_eq!(body["error"]["code"], "account_frozen");
    server
        .db()
        .await
        .execute("DELETE FROM sessions WHERE device_id = $1", &[&sess.device_id])
        .await
        .unwrap();
    let (status, body) = login_with_device(&server, &account, "Desktop", sess.device_id).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["account"]["entitlement"], "frozen");
    let token = body["token"].as_str().unwrap().to_string();
    let sess = Sess { token, ..sess };

    // Reads and the ways out keep working.
    let pulled = pull(&server, &sess, 0).await;
    assert_eq!(pulled["account"]["entitlement"], "frozen");
    assert_eq!(pulled["changes"].as_array().unwrap().len(), 1, "the item is still served");
    assert_eq!(server.get_as("/v1/vault/header", &sess).send().await.unwrap().status(), 200);
    assert_eq!(server.get_as("/v1/devices", &sess).send().await.unwrap().status(), 200);
    let res = server
        .post_as("/v1/account/delete", &sess)
        .json(&json!({
            "currentAuthKey": BASE64.encode(&account.auth_key),
            "email": account.email,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204, "deletion is never blocked");
    server.cleanup().await;
}

#[tokio::test]
async fn a_frozen_account_cannot_approve_a_new_device() {
    let server = TestServer::start().await;
    let (account, phone) = signed_in(&server, "user@example.com").await;
    let desktop = Uuid::new_v4();
    let res = server
        .post("/v1/pairings")
        .json(&json!({
            "deviceId": desktop,
            "deviceName": "Desktop · Linux",
            "claimHash": BASE64URL_NOPAD.encode(&Sha256::digest([7u8; 32])),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let id = res.json::<Value>().await.unwrap()["pairingId"]
        .as_str()
        .unwrap()
        .to_string();
    set_plan(&server, account.account_id, Status::Frozen).await;
    let status = server
        .post_as(&format!("/v1/pairings/{id}/approve"), &phone)
        .json(&json!({ "envelope": BASE64.encode(&[1u8; 200]) }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, 402);
    let registered: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM devices WHERE id = $1", &[&desktop])
        .await
        .unwrap()
        .get(0);
    assert_eq!(registered, 0, "a refused approval registers nothing");
    server.cleanup().await;
}

#[tokio::test]
async fn a_trial_past_its_end_is_frozen_without_any_job_running() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    set_plan(&server, account.account_id, Status::Trialing).await;
    let db = server.db().await;
    db.execute(
        "UPDATE subscriptions SET trial_ends_at = now() - interval '1 minute' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let (status, _) = write(&server, &sess, vec![change(Uuid::new_v4(), None, b"o", b"d")]).await;
    assert_eq!(status, 402);
    db.execute(
        "UPDATE subscriptions SET trial_ends_at = now() + interval '1 day' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let (status, _) = write(&server, &sess, vec![change(Uuid::new_v4(), None, b"o", b"d")]).await;
    assert_eq!(status, 200);
    let pulled = pull(&server, &sess, 0).await;
    assert_eq!(pulled["account"]["status"], "trialing");
    assert!(pulled["account"]["trialEndsAt"].is_string());
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn past_due_is_full_during_grace_then_frozen() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    let db = server.db().await;
    havenkeys_server::billing::set_status(
        &db,
        account.account_id,
        havenkeys_server::billing::Actor::Provider,
        Status::PastDue,
        Some(chrono::Utc::now() + chrono::Duration::days(7)),
        "payment failed",
    )
    .await
    .unwrap();
    let (status, _) = write(&server, &sess, vec![change(Uuid::new_v4(), None, b"o", b"d")]).await;
    assert_eq!(status, 200);
    db.execute(
        "UPDATE subscriptions SET grace_ends_at = now() - interval '1 minute' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let (status, _) = write(&server, &sess, vec![change(Uuid::new_v4(), None, b"o", b"d")]).await;
    assert_eq!(status, 402);
    let events: i64 = db
        .query_one(
            "SELECT count(*) FROM billing_events WHERE account_id = $1 AND actor = 'provider' AND new_status = 'past_due'",
            &[&account.account_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(events, 1);
    drop(db);
    server.cleanup().await;
}
```

`chrono` is already a dependency; add `chrono` to `[dev-dependencies]` only if `cargo test` complains it is not visible to tests (it is a normal dependency, so it is visible).

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p havenkeys-server --test plans`
Expected: the tests compile (`set_plan` calls `billing::set_status` from Task 1) and FAIL: writes answer 200, logins from new devices answer 200, no `account` object in the bodies.

- [ ] **Step 3: Add the error**

In `crates/havenkeys-server/src/error.rs`, add the variant after `AccountDeleted`:

```rust
    /// The account's trial ended or its payment lapsed (spec 2026-10-07
    /// §5.4). Reads, export and deletion still work; writes and new devices
    /// do not.
    AccountFrozen,
```

and in `parts`, after the `AccountDeleted` arm:

```rust
            Self::AccountFrozen => (
                StatusCode::PAYMENT_REQUIRED,
                "account_frozen",
                "This account is frozen: the trial ended or payment lapsed. Reading and export still work.",
            ),
```

- [ ] **Step 4: Make the session carry the entitlement**

In `crates/havenkeys-server/src/auth/mod.rs`:

```rust
use crate::billing::{self, Entitlement, Subscription};
```

Extend the struct and add the method:

```rust
pub struct Session {
    pub account_id: Uuid,
    pub device_id: Uuid,
    pub vault_id: Uuid,
    pub token_hash: Vec<u8>,
    /// The plan row, for the `account` object in answers.
    pub subscription: Option<Subscription>,
    /// Computed once per request from the row and the clock.
    pub entitlement: Entitlement,
}

impl Session {
    /// Refuse a write for a frozen account (spec 2026-10-07 §5.4). Called
    /// first thing by every route that changes the account or its vault.
    pub fn require_full(&self) -> Result<(), ApiError> {
        match self.entitlement {
            Entitlement::Full => Ok(()),
            Entitlement::Frozen => Err(ApiError::AccountFrozen),
        }
    }
}
```

Change the extractor's query to a LEFT JOIN and fill the fields:

```rust
        let row = db
            .query_opt(
                "SELECT s.account_id, s.device_id, v.id,
                        p.status, p.trial_ends_at, p.current_period_end, p.grace_ends_at
                   FROM sessions s
                   JOIN accounts a ON a.id = s.account_id
                   JOIN devices  d ON d.id = s.device_id
                   JOIN vaults   v ON v.account_id = s.account_id
                   LEFT JOIN subscriptions p ON p.account_id = s.account_id
                  WHERE s.token_hash = $1
                    AND s.expires_at > now()
                    AND a.status = 'active'
                    AND d.revoked_at IS NULL",
                &[&hash],
            )
            .await?;
```

and at the end:

```rust
        let subscription = billing::from_row(&row, 3);
        let entitlement = billing::entitlement_of(subscription.as_ref(), Utc::now());
        Ok(Session {
            account_id: row.get(0),
            device_id,
            vault_id: row.get(2),
            token_hash: hash,
            subscription,
            entitlement,
        })
```

- [ ] **Step 5: Gate the three authenticated write points**

`crates/havenkeys-server/src/routes/items.rs`, first line of `write`'s body, before `check_batch`:

```rust
    session.require_full()?;
```

`crates/havenkeys-server/src/routes/account.rs`, first line of `change_credentials`'s body, before `check_header`:

```rust
    session.require_full()?;
```

`crates/havenkeys-server/src/routes/pairings.rs`, first line of `approve`'s body, before `pairing_id`:

```rust
    session.require_full()?;
```

`delete_account` is deliberately not gated.

- [ ] **Step 6: Login refuses a new device when frozen and answers the `account` object**

In `crates/havenkeys-server/src/routes/auth.rs`, add `use crate::billing;` and, in `login`, replace the block from `keys.clear(&db).await?;` to the final `Ok(...)` with:

```rust
    keys.clear(&db).await?;
    // A frozen account keeps the devices it has and gains none (spec
    // 2026-10-07 §5.4). Decided here, not in `register_device`, which the
    // pairing approval also uses and which must not learn about plans.
    let subscription = billing::load(&db, account_id).await?;
    let now = chrono::Utc::now();
    let known = db
        .query_opt(
            "SELECT 1 FROM devices WHERE id = $1 AND account_id = $2 AND revoked_at IS NULL",
            &[&req.device_id, &account_id],
        )
        .await?
        .is_some();
    if !known && billing::entitlement_of(subscription.as_ref(), now) == billing::Entitlement::Frozen {
        tracing::info!(account_id = %account_id, outcome = "frozen", "login");
        return Err(ApiError::AccountFrozen);
    }
    register_device(&db, account_id, req.device_id, &device_name).await?;
    db.execute(
        "DELETE FROM sessions WHERE device_id = $1",
        &[&req.device_id],
    )
    .await?;
    let (token, expires) = auth::issue_token(&db, account_id, req.device_id).await?;

    tracing::info!(account_id = %account_id, device_id = %req.device_id, outcome = "accepted", "login");
    Ok(axum::Json(serde_json::json!({
        "token": token.as_str(),
        "expiresAt": expires.to_rfc3339(),
        "vaultId": vault_id,
        "account": billing::account_json(subscription.as_ref(), now),
    })))
```

- [ ] **Step 7: Sync answers the `account` object**

In `crates/havenkeys-server/src/routes/sync.rs`, add `use crate::billing;` and change the final answer of `pull` to:

```rust
    Ok(axum::Json(serde_json::json!({
        "cursor": cursor,
        "hasMore": has_more,
        "changes": changes,
        "account": billing::account_json(session.subscription.as_ref(), chrono::Utc::now()),
    })))
```

- [ ] **Step 8: Run the tests**

Run: `cargo test -p havenkeys-server --test plans --test isolation --test sync --test auth --test pairings --test deletion`
Expected: the four `plans` tests PASS and every pre-existing test PASSES. (An account created by `admin new-account` has no plan row until Task 3, which the extractor treats as `Full`, so nothing changes for the existing tests.)

- [ ] **Step 9: Commit**

```bash
git add crates/havenkeys-server/src crates/havenkeys-server/tests
git commit -m "feat(server): frozen accounts get 402 on writes and new devices; login and sync carry the account object"
```

---

### Task 3: Admin CLI, activation starts the trial, the gateway seam

**Files:**
- Modify: `crates/havenkeys-server/src/admin.rs`
- Modify: `crates/havenkeys-server/src/routes/accounts.rs:182-204` (after the `UPDATE accounts` in `activate`)
- Create: `crates/havenkeys-server/src/billing/provider.rs` (replace the placeholder)
- Modify: `crates/havenkeys-server/tests/support/mod.rs:168-181` (`new_invite` passes `trial: false`)
- Test: `crates/havenkeys-server/tests/admin.rs`, `crates/havenkeys-server/tests/plans.rs`

**Interfaces:**
- Consumes: `billing::{set_status, load, Actor, Status, TRIAL_DAYS}`.
- Produces:
  - `AdminCommand::NewAccount { email, server_url, trial: bool }`; `AdminCommand::SetPlan { email, status: billing::Status, until: Option<String> }`.
  - `billing::provider::ProviderEvent { provider, customer, reference, status, period_end, reason }` and `apply_provider_event(db, account_id, event)`.
  - `admin::check_server_url` moves to `config::check_public_url` (re-exported name used by Task 4).

- [ ] **Step 1: Write the failing tests**

In `crates/havenkeys-server/tests/admin.rs`, update the helper and add tests:

```rust
fn new_account(email: &str) -> AdminCommand {
    AdminCommand::NewAccount {
        email: email.into(),
        server_url: "https://vault.example.com".into(),
        trial: false,
    }
}

#[tokio::test]
async fn new_account_is_complimentary_by_default_and_trialing_on_request() {
    let server = support::TestServer::start().await;
    let printed = admin::run(new_account("free@example.com"), server.pool())
        .await
        .unwrap();
    let free = invite::decode(printed.trim()).unwrap().account;
    let printed = admin::run(
        AdminCommand::NewAccount {
            email: "trial@example.com".into(),
            server_url: "https://vault.example.com".into(),
            trial: true,
        },
        server.pool(),
    )
    .await
    .unwrap();
    let trial = invite::decode(printed.trim()).unwrap().account;

    let db = server.db().await;
    for (id, expected) in [(free, "complimentary"), (trial, "trialing")] {
        let row = db
            .query_one(
                "SELECT status, trial_ends_at FROM subscriptions WHERE account_id = $1",
                &[&id],
            )
            .await
            .unwrap();
        assert_eq!(row.get::<_, String>(0), expected);
        assert!(
            row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(1).is_none(),
            "the trial starts at activation, not at invitation"
        );
        let events: i64 = db
            .query_one(
                "SELECT count(*) FROM billing_events WHERE account_id = $1 AND actor = 'admin'",
                &[&id],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(events, 1);
    }
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn set_plan_changes_the_status_and_records_it() {
    let server = support::TestServer::start().await;
    let (account, _) = support::signed_in(&server, "user@example.com").await;
    let out = admin::run(
        AdminCommand::SetPlan {
            email: "User@Example.com".into(),
            status: havenkeys_server::billing::Status::Active,
            until: Some("2027-01-31".into()),
        },
        server.pool(),
    )
    .await
    .unwrap();
    assert_eq!(out, "active until 2027-01-31T00:00:00+00:00");
    let db = server.db().await;
    let row = db
        .query_one(
            "SELECT status, current_period_end::text FROM subscriptions WHERE account_id = $1",
            &[&account.account_id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "active");
    assert!(row.get::<_, String>(1).starts_with("2027-01-31"));
    let last: (Option<String>, String) = db
        .query_one(
            "SELECT old_status, new_status FROM billing_events WHERE account_id = $1 ORDER BY id DESC LIMIT 1",
            &[&account.account_id],
        )
        .await
        .map(|r| (r.get(0), r.get(1)))
        .unwrap();
    assert_eq!(last, (Some("complimentary".into()), "active".into()));
    assert!(admin::run(
        AdminCommand::SetPlan {
            email: "nobody@example.com".into(),
            status: havenkeys_server::billing::Status::Frozen,
            until: None,
        },
        server.pool(),
    )
    .await
    .is_err());
    assert!(admin::run(
        AdminCommand::SetPlan {
            email: "user@example.com".into(),
            status: havenkeys_server::billing::Status::Active,
            until: Some("next tuesday".into()),
        },
        server.pool(),
    )
    .await
    .is_err());
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn list_accounts_shows_the_plan() {
    let server = support::TestServer::start().await;
    support::signed_in(&server, "user@example.com").await;
    let out = admin::run(AdminCommand::ListAccounts, server.pool())
        .await
        .unwrap();
    assert!(out.contains("user@example.com  active  complimentary"), "{out}");
    server.cleanup().await;
}
```

Add to `crates/havenkeys-server/tests/plans.rs`:

```rust
#[tokio::test]
async fn an_existing_account_is_complimentary_and_full() {
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    let pulled = pull(&server, &sess, 0).await;
    assert_eq!(pulled["account"]["status"], "complimentary");
    assert_eq!(pulled["account"]["entitlement"], "full");
    assert!(pulled["account"]["trialEndsAt"].is_null());
    let (status, body) = login_with_device(&server, &account, "Phone", Uuid::new_v4()).await;
    assert_eq!(status, 200);
    assert_eq!(body["account"]["entitlement"], "full");
    server.cleanup().await;
}

#[tokio::test]
async fn activation_starts_a_fourteen_day_trial_for_a_trialing_account() {
    let server = TestServer::start().await;
    let printed = havenkeys_server::admin::run(
        havenkeys_server::admin::AdminCommand::NewAccount {
            email: "trial@example.com".into(),
            server_url: "https://vault.example.com".into(),
            trial: true,
        },
        server.pool(),
    )
    .await
    .unwrap();
    let invite = printed.trim().to_string();
    let vault_id = Uuid::new_v4();
    let res = server
        .post("/v1/accounts/activate")
        .json(&activate_body("trial@example.com", &invite, vault_id, &[5u8; 32]))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let account_id: Uuid = res.json::<Value>().await.unwrap()["accountId"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let days: f64 = server
        .db()
        .await
        .query_one(
            "SELECT extract(epoch FROM trial_ends_at - now()) / 86400 FROM subscriptions WHERE account_id = $1",
            &[&account_id],
        )
        .await
        .unwrap()
        .get(0);
    assert!((13.9..=14.0).contains(&days), "{days}");
    server.cleanup().await;
}

#[tokio::test]
async fn a_provider_event_is_the_only_other_writer() {
    use havenkeys_server::billing::provider::{apply_provider_event, ProviderEvent};
    let server = TestServer::start().await;
    let (account, sess) = signed_in(&server, "user@example.com").await;
    let db = server.db().await;
    apply_provider_event(
        &db,
        account.account_id,
        ProviderEvent {
            provider: "stripe",
            customer: "cus_123".into(),
            reference: "sub_456".into(),
            status: Status::Active,
            period_end: Some(chrono::Utc::now() - chrono::Duration::days(1)),
            reason: "invoice.payment_failed".into(),
        },
    )
    .await
    .unwrap();
    let row = db
        .query_one(
            "SELECT status, provider, provider_customer, provider_ref FROM subscriptions WHERE account_id = $1",
            &[&account.account_id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "active");
    assert_eq!(row.get::<_, Option<String>>(1).as_deref(), Some("stripe"));
    assert_eq!(row.get::<_, Option<String>>(2).as_deref(), Some("cus_123"));
    assert_eq!(row.get::<_, Option<String>>(3).as_deref(), Some("sub_456"));
    let (status, _) = write(&server, &sess, vec![change(Uuid::new_v4(), None, b"o", b"d")]).await;
    assert_eq!(status, 402, "active with a period end in the past is frozen");
    let actor: String = db
        .query_one(
            "SELECT actor FROM billing_events WHERE account_id = $1 ORDER BY id DESC LIMIT 1",
            &[&account.account_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(actor, "provider");
    drop(db);
    server.cleanup().await;
}
```

In `tests/support/mod.rs`, `new_invite`: add `trial: false,` to the `AdminCommand::NewAccount` literal.

Append to `crates/havenkeys-server/tests/schema.rs`:

```rust
/// Every account that existed before plans is complimentary (the migration
/// for old rows, the admin CLI for new ones), and an unknown plan status or
/// origin is impossible.
#[tokio::test]
async fn plans_schema_is_constrained_and_existing_accounts_are_complimentary() {
    let server = support::TestServer::start().await;
    let (account, _) = support::signed_in(&server, "old@example.com").await;
    let db = server.db().await;
    let status: String = db
        .query_one(
            "SELECT status FROM subscriptions WHERE account_id = $1",
            &[&account.account_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(status, "complimentary");
    let bad = db
        .execute(
            "UPDATE subscriptions SET status = 'gold' WHERE account_id = $1",
            &[&account.account_id],
        )
        .await;
    assert!(bad.is_err(), "the status list is closed");
    let bad = db
        .execute(
            "UPDATE accounts SET created_by = 'robot' WHERE id = $1",
            &[&account.account_id],
        )
        .await;
    assert!(bad.is_err());
    drop(db);
    server.cleanup().await;
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p havenkeys-server --test admin --test plans --test schema`
Expected: compile errors (`trial` field, `SetPlan`, `provider` module items missing).

- [ ] **Step 3: Move `check_server_url` to `config.rs`**

In `crates/havenkeys-server/src/config.rs`, add (public, with its test moved along):

```rust
/// A URL clients will be told to trust: an invite carries it and a client
/// sends its master-password proof there. HTTPS, or an explicit localhost
/// for development.
pub fn check_public_url(raw: &str) -> Result<String, String> {
    let url = raw.trim().trim_end_matches('/').to_string();
    let local = url.starts_with("http://localhost") || url.starts_with("http://127.0.0.1");
    if !url.starts_with("https://") && !local {
        return Err("the server URL must be https (or http on localhost)".into());
    }
    if url.len() > 512 {
        return Err("the server URL is too long".into());
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::check_public_url;

    #[test]
    fn a_plain_http_server_url_is_refused() {
        assert!(check_public_url("http://vault.example.com").is_err());
        assert_eq!(
            check_public_url("https://vault.example.com/").unwrap(),
            "https://vault.example.com"
        );
        assert!(check_public_url("http://localhost:8080").is_ok());
    }
}
```

In `admin.rs`, delete `check_server_url` and its test module, and call `crate::config::check_public_url(server_url)?` instead.

- [ ] **Step 4: Rewrite the admin CLI**

Replace the enum and `run` in `crates/havenkeys-server/src/admin.rs`:

```rust
use crate::billing::{self, Actor, Status};
use crate::invite::{self, Invite, INVITE_TTL_DAYS};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use clap::Subcommand;
use deadpool_postgres::Pool;
use uuid::Uuid;

#[derive(Subcommand, Debug)]
pub enum AdminCommand {
    /// Create an account and print its single-use invite string.
    NewAccount {
        #[arg(long)]
        email: String,
        /// The public URL clients should talk to, carried in the invite.
        #[arg(long = "server-url")]
        server_url: String,
        /// Start a 14-day trial at activation instead of a complimentary
        /// plan with no end date.
        #[arg(long)]
        trial: bool,
    },
    /// List accounts: id, email, status, plan status, activation time.
    ListAccounts,
    /// Delete an account and everything it owns. Irreversible.
    DeleteAccount {
        #[arg(long)]
        email: String,
    },
    /// Set an account's plan status, recording who did it.
    SetPlan {
        #[arg(long)]
        email: String,
        #[arg(long, value_enum)]
        status: Status,
        /// When the status ends: a date (`2027-01-31`, midnight UTC) or an
        /// RFC 3339 time. Read as the trial end for `trialing`, the paid
        /// period end for `active`, the grace end for `past-due`.
        #[arg(long)]
        until: Option<String>,
    },
}

/// Returns what the operator should see on stdout. Returning it rather than
/// printing keeps the invite out of this crate's logs and lets a test read it.
pub async fn run(cmd: AdminCommand, pool: &Pool) -> Result<String, String> {
    match cmd {
        AdminCommand::NewAccount {
            email,
            server_url,
            trial,
        } => new_account(pool, &email, &server_url, trial).await,
        AdminCommand::ListAccounts => list_accounts(pool).await,
        AdminCommand::DeleteAccount { email } => delete_account(pool, &email).await,
        AdminCommand::SetPlan {
            email,
            status,
            until,
        } => set_plan(pool, &email, status, until.as_deref()).await,
    }
}

/// Create an invited account with its plan row and return its invite string.
async fn new_account(
    pool: &Pool,
    email: &str,
    server_url: &str,
    trial: bool,
) -> Result<String, String> {
    let email = crate::email::normalize(email)?.to_string();
    let server_url = crate::config::check_public_url(server_url)?;
    let account = Uuid::new_v4();
    let secret = invite::generate_secret();
    let now = Utc::now();
    let mut client = pool.get().await.map_err(|_| "no database".to_string())?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| "could not create the account".to_string())?;
    tx.execute(
        "INSERT INTO accounts
           (id, email_normalized, status, invite_hash, invite_expires_at, created_at, created_by)
         VALUES ($1, $2, 'invited', $3, $4, $5, 'admin')",
        &[
            &account,
            &email,
            &invite::hash(&secret).to_vec(),
            &(now + Duration::days(INVITE_TTL_DAYS)),
            &now,
        ],
    )
    .await
    .map_err(|e| match e.code().map(|c| c.code().to_string()) {
        Some(code) if code == "23505" => "an account with that email already exists".to_string(),
        _ => "could not create the account".to_string(),
    })?;
    let status = if trial {
        Status::Trialing
    } else {
        Status::Complimentary
    };
    billing::set_status(&tx, account, Actor::Admin, status, None, "new-account")
        .await
        .map_err(|_| "could not create the account".to_string())?;
    tx.commit()
        .await
        .map_err(|_| "could not create the account".to_string())?;
    Ok(invite::encode(&Invite {
        server: server_url,
        email,
        account,
        secret: secret.to_string(),
    }))
}

/// One line per account: id, email, status, plan status, activation time.
async fn list_accounts(pool: &Pool) -> Result<String, String> {
    let client = pool.get().await.map_err(|_| "no database".to_string())?;
    let rows = client
        .query(
            "SELECT a.id, a.email_normalized, a.status, coalesce(p.status, '-'), a.activated_at
               FROM accounts a LEFT JOIN subscriptions p ON p.account_id = a.id
              ORDER BY a.created_at",
            &[],
        )
        .await
        .map_err(|_| "could not list accounts".to_string())?;
    Ok(rows
        .iter()
        .map(|row| {
            let id: Uuid = row.get(0);
            let email: String = row.get(1);
            let status: String = row.get(2);
            let plan: String = row.get(3);
            let at: Option<DateTime<Utc>> = row.get(4);
            format!(
                "{id}  {email}  {status}  {plan}  {}",
                at.map(|t| t.to_rfc3339()).unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

async fn set_plan(
    pool: &Pool,
    email: &str,
    status: Status,
    until: Option<&str>,
) -> Result<String, String> {
    let email = crate::email::normalize(email)?;
    let until = until.map(parse_until).transpose()?;
    let failed = |_| "could not set the plan".to_string();
    let mut client = pool.get().await.map_err(|_| "no database".to_string())?;
    let tx = client.transaction().await.map_err(failed)?;
    let id: Uuid = tx
        .query_opt(
            "SELECT id FROM accounts WHERE email_normalized = $1 FOR UPDATE",
            &[&email],
        )
        .await
        .map_err(failed)?
        .ok_or_else(|| "no such account".to_string())?
        .get(0);
    billing::set_status(&tx, id, Actor::Admin, status, until, "set-plan")
        .await
        .map_err(failed)?;
    tx.commit().await.map_err(failed)?;
    Ok(match until {
        Some(t) => format!("{} until {}", status.as_str(), t.to_rfc3339()),
        None => status.as_str().to_string(),
    })
}

/// `YYYY-MM-DD` (midnight UTC) or RFC 3339.
fn parse_until(raw: &str) -> Result<DateTime<Utc>, String> {
    if let Ok(t) = DateTime::parse_from_rfc3339(raw.trim()) {
        return Ok(t.with_timezone(&Utc));
    }
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|t| t.and_utc())
        .ok_or_else(|| "--until must be a date (2027-01-31) or an RFC 3339 time".to_string())
}
```

Keep `delete_account` as it is (it already takes `pool`). Remove the now-unused `Object` import. Update the module doc comment's first paragraph to: "Account provisioning and plans. Signup (`routes::signup`) creates accounts too when the operator opens it; this CLI is the other way, and the only one on a server where signup is off."

- [ ] **Step 5: Activation starts the trial**

In `crates/havenkeys-server/src/routes/accounts.rs`, `activate`, after the `UPDATE accounts ... activated_at = now()` statement and before the `INSERT INTO vaults`:

```rust
    // The trial starts now, not when the invite was made (spec 2026-10-07
    // §5.1). A complimentary row is left alone.
    tx.execute(
        "UPDATE subscriptions
            SET trial_ends_at = now() + make_interval(days => $2), updated_at = now()
          WHERE account_id = $1 AND status = 'trialing' AND trial_ends_at IS NULL",
        &[&parsed.account, &crate::billing::TRIAL_DAYS],
    )
    .await?;
```

- [ ] **Step 6: The provider seam**

Replace the placeholder `crates/havenkeys-server/src/billing/provider.rs`:

```rust
//! Where a payment gateway's webhook will land (spec 2026-10-07 §5.5).
//!
//! Nothing calls this yet. When Stripe or Mercado Pago is wired up, its
//! webhook route verifies the signature, maps the event to a
//! `ProviderEvent`, and calls `apply_provider_event` inside a transaction.
//! Routes never touch `subscriptions` directly.

use super::{set_status, Actor, Status};
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use uuid::Uuid;

pub struct ProviderEvent {
    /// `"stripe"` or `"mercadopago"`.
    pub provider: &'static str,
    pub customer: String,
    /// The gateway's subscription id.
    pub reference: String,
    pub status: Status,
    /// Paid until, for `Active`; the grace end, for `PastDue`.
    pub period_end: Option<DateTime<Utc>>,
    /// The gateway's event name, for the audit trail.
    pub reason: String,
}

pub async fn apply_provider_event(
    db: &impl GenericClient,
    account_id: Uuid,
    event: ProviderEvent,
) -> Result<(), tokio_postgres::Error> {
    set_status(
        db,
        account_id,
        Actor::Provider,
        event.status,
        event.period_end,
        &event.reason,
    )
    .await?;
    db.execute(
        "UPDATE subscriptions
            SET provider = $2, provider_customer = $3, provider_ref = $4
          WHERE account_id = $1",
        &[&account_id, &event.provider, &event.customer, &event.reference],
    )
    .await?;
    Ok(())
}
```

- [ ] **Step 7: Run the tests**

Run: `cargo test -p havenkeys-server`
Expected: all PASS. `no_logging` still passes (no new log fields carry values).

- [ ] **Step 8: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): admin new-account --trial and set-plan, trial starts at activation, provider seam"
```

---

### Task 4: Mail: the `Mailer` trait, SMTP over lettre, templates, configuration

**Files:**
- Modify: `crates/havenkeys-server/Cargo.toml` (dependencies `lettre`, `hmac`)
- Create: `crates/havenkeys-server/src/mail/mod.rs`
- Create: `crates/havenkeys-server/src/mail/templates.rs`
- Modify: `crates/havenkeys-server/src/config.rs`
- Modify: `crates/havenkeys-server/src/lib.rs` (`pub mod mail;`)
- Modify: `crates/havenkeys-server/src/routes/mod.rs:25-44` (`AppState.mailer`, `AppState.signup_url`)
- Modify: `crates/havenkeys-server/src/main.rs:73-90` (build the mailer)
- Modify: `crates/havenkeys-server/tests/support/mod.rs:39-46` (new state fields)
- Test: unit tests in `mail/templates.rs` and `config.rs`

**Interfaces:**
- Produces:
  - `mail::Mail { to: String, subject: String, body: String }`.
  - `mail::Mailer` trait: `fn send(&self, mail: Mail) -> BoxFuture<'_, Result<(), MailError>>`.
  - `mail::Smtp::new(url: &str, from: &str) -> Result<Smtp, String>`.
  - `mail::Recording` (`Default`): `sent(&self) -> Vec<Mail>`, `fail: AtomicBool`.
  - `mail::templates::Locale { En, PtBr }` with `parse(&str) -> Option<Locale>` (accepts `"en"`, `"pt-BR"`, `"pt-br"`) and `from_db(Option<&str>) -> Locale`.
  - `templates::signup_code(locale, code) -> (String, String)`, `signup_invite(locale, invite)`, `already_registered(locale)`, `trial_ending(locale, days: i64)`, `trial_ended(locale)`; each returns `(subject, body)`.
  - `Config { signup_open: bool, public_url: Option<String>, smtp: Option<SmtpConfig { url: Zeroizing<String>, from: String }> }`.
  - `AppState { mailer: Option<Arc<dyn Mailer>>, signup_url: Option<String> }` (`signup_url` is `Some` exactly when signup is open).

- [ ] **Step 1: Add the dependencies**

In `crates/havenkeys-server/Cargo.toml`, under `[dependencies]`:

```toml
# SMTP for the signup code, the invite and the trial notices. rustls with
# ring, like the Postgres connection: one TLS stack in the binary.
lettre = { version = "0.11", default-features = false, features = [
  "builder",
  "smtp-transport",
  "tokio1",
  "tokio1-rustls",
  "ring",
  "webpki-roots",
  "hostname",
] }
# HMAC-SHA-256 keys the stored signup code with the server secret.
hmac = "0.12"
```

Run: `cargo build -p havenkeys-server && cargo tree -p havenkeys-server -i aws-lc-rs`
Expected: the build succeeds and the tree command prints `error: package ID specification 'aws-lc-rs' did not match any packages` (nothing pulled aws-lc-rs). If it prints a dependency path instead, replace `"ring"` in the feature list with `"rustls-no-provider", "ring"` and run both commands again; the second form is what lettre 0.11.23 uses when `rustls` would otherwise choose its default provider.

Then: `cargo deny check licenses` must pass (lettre is MIT; its tree is MIT/Apache/ISC). If a new license shows up, add it to `deny.toml` only after reading it.

- [ ] **Step 2: Write the failing template and config tests**

Create `crates/havenkeys-server/src/mail/templates.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_parse_as_the_site_sends_them() {
        assert_eq!(Locale::parse("en"), Some(Locale::En));
        assert_eq!(Locale::parse("pt-BR"), Some(Locale::PtBr));
        assert_eq!(Locale::parse("pt-br"), Some(Locale::PtBr));
        assert_eq!(Locale::parse("fr"), None);
        assert_eq!(Locale::from_db(None), Locale::En);
        assert_eq!(Locale::from_db(Some("pt-BR")), Locale::PtBr);
    }

    #[test]
    fn the_code_and_the_invite_stand_on_their_own_line() {
        for locale in [Locale::En, Locale::PtBr] {
            let (subject, body) = signup_code(locale, "123456");
            assert!(!subject.contains("123456"), "codes stay out of subjects");
            assert!(body.lines().any(|l| l.trim() == "123456"), "{body}");
            let (_, body) = signup_invite(locale, "HKINV1-abc");
            assert!(body.lines().any(|l| l.trim() == "HKINV1-abc"));
        }
    }

    #[test]
    fn every_email_exists_in_both_languages_and_is_plain_text() {
        for locale in [Locale::En, Locale::PtBr] {
            for (subject, body) in [
                signup_code(locale, "000000"),
                signup_invite(locale, "HKINV1-x"),
                already_registered(locale),
                trial_ending(locale, 3),
                trial_ended(locale),
            ] {
                assert!(!subject.is_empty() && !body.is_empty());
                assert!(!body.contains('<'), "no HTML: {body}");
                assert!(!body.contains("utm_"), "no tracking: {body}");
            }
        }
        assert!(trial_ending(Locale::En, 3).0.contains("3 days"));
        assert!(trial_ending(Locale::PtBr, 1).0.contains("1 dia"));
    }
}
```

In `crates/havenkeys-server/src/config.rs`, add to the test module from Task 3:

```rust
    #[test]
    fn signup_open_needs_smtp_and_a_public_url() {
        let base = |signup: &str| {
            vec![
                ("DATABASE_URL", "postgres://x".to_string()),
                ("SERVER_SECRET", data_encoding::BASE64.encode(&[1u8; 32])),
                ("HAVENKEYS_SIGNUP", signup.to_string()),
            ]
        };
        let off = Config::from_vars(&base("off")).unwrap();
        assert!(!off.signup_open && off.smtp.is_none());
        assert!(Config::from_vars(&base("open")).is_err(), "no SMTP, no URL");
        let mut vars = base("open");
        vars.push(("SMTP_URL", "smtps://u:p@smtp.example.com:465".into()));
        vars.push(("SMTP_FROM", "HavenKeys <no-reply@havenkeys.net>".into()));
        assert!(Config::from_vars(&vars).is_err(), "no public URL");
        vars.push(("HAVENKEYS_PUBLIC_URL", "https://api.havenkeys.net/".into()));
        let open = Config::from_vars(&vars).unwrap();
        assert!(open.signup_open);
        assert_eq!(open.public_url.as_deref(), Some("https://api.havenkeys.net"));
        assert_eq!(open.smtp.as_ref().unwrap().from, "HavenKeys <no-reply@havenkeys.net>");
        assert!(Config::from_vars(&base("maybe")).is_err(), "only open or off");
        let mut smtp_only = base("off");
        smtp_only.push(("SMTP_URL", "smtps://u:p@smtp.example.com:465".into()));
        smtp_only.push(("SMTP_FROM", "no-reply@havenkeys.net".into()));
        assert!(Config::from_vars(&smtp_only).unwrap().smtp.is_some(), "trial mail without signup");
    }
```

- [ ] **Step 3: Run them to see them fail**

Run: `cargo test -p havenkeys-server --lib mail config`
Expected: compile errors (`mail` module, `Config::from_vars` missing).

- [ ] **Step 4: Write the mail module**

`crates/havenkeys-server/src/mail/mod.rs`:

```rust
//! Outbound email: the signup code, the invite, and account notices.
//!
//! Everything goes through the `Mailer` trait so the routes and the daily
//! task can be tested with the in-memory `Recording` sender, and so the
//! SMTP library stays in one file. Bodies are plain text; nothing in them
//! is tracked, and no address, code or invite is ever logged (CLAUDE.md
//! §40): a send failure is logged by kind only.

pub mod templates;

use lettre::message::Mailbox;
use lettre::transport::smtp::AsyncSmtpTransport;
use lettre::{AsyncTransport, Message, Tokio1Executor};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    pub to: String,
    pub subject: String,
    pub body: String,
}

/// The SMTP error can quote the recipient, so it is never carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailError;

pub trait Mailer: Send + Sync {
    fn send(&self, mail: Mail) -> BoxFuture<'_, Result<(), MailError>>;
}

pub struct Smtp {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl Smtp {
    /// `url` is what `SMTP_URL` holds: `smtps://user:pass@host:465` for
    /// implicit TLS or `smtp://user:pass@host:587?tls=required` for
    /// STARTTLS. Plain `smtp://` without `tls=` is refused: the code it
    /// carries is a credential for the account.
    pub fn new(url: &str, from: &str) -> Result<Self, String> {
        let plain = url.starts_with("smtp://") && !url.contains("tls=required");
        if plain {
            return Err("SMTP_URL must use smtps:// or smtp://…?tls=required".into());
        }
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url)
            .map_err(|_| "SMTP_URL is not a valid SMTP URL".to_string())?
            .build();
        let from = from
            .parse::<Mailbox>()
            .map_err(|_| "SMTP_FROM is not a valid address".to_string())?;
        Ok(Self { transport, from })
    }
}

impl Mailer for Smtp {
    fn send(&self, mail: Mail) -> BoxFuture<'_, Result<(), MailError>> {
        Box::pin(async move {
            let to = mail.to.parse::<Mailbox>().map_err(|_| MailError)?;
            let message = Message::builder()
                .from(self.from.clone())
                .to(to)
                .subject(mail.subject)
                .body(mail.body)
                .map_err(|_| MailError)?;
            match self.transport.send(message).await {
                Ok(_) => Ok(()),
                Err(err) => {
                    // Kind only: lettre's message can carry the address.
                    let kind = if err.is_transient() {
                        "transient"
                    } else if err.is_permanent() {
                        "permanent"
                    } else {
                        "transport"
                    };
                    tracing::warn!(kind, "mail not sent");
                    Err(MailError)
                }
            }
        })
    }
}

/// Keeps every mail in memory. Tests read the code and the invite from it;
/// `fail` makes every send fail, to test the routes' behaviour when SMTP
/// is down.
#[derive(Default)]
pub struct Recording {
    mails: Mutex<Vec<Mail>>,
    pub fail: AtomicBool,
}

impl Recording {
    pub fn sent(&self) -> Vec<Mail> {
        self.mails.lock().unwrap().clone()
    }
}

impl Mailer for Recording {
    fn send(&self, mail: Mail) -> BoxFuture<'_, Result<(), MailError>> {
        Box::pin(async move {
            if self.fail.load(Ordering::SeqCst) {
                return Err(MailError);
            }
            self.mails.lock().unwrap().push(mail);
            Ok(())
        })
    }
}
```

If `err.is_transient()` / `is_permanent()` do not exist on `lettre::transport::smtp::Error` in the resolved version, use `"transport"` for every case; the log line must stay a fixed string.

- [ ] **Step 5: Write the templates**

Above the tests in `crates/havenkeys-server/src/mail/templates.rs`:

```rust
//! The five emails, in English and Brazilian Portuguese. Plain text, no
//! links other than the site's own pages, no tracking. Subjects never
//! carry the code or the invite: notification previews show subjects.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    PtBr,
}

impl Locale {
    /// As the site sends it in `signup/start`.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "en" => Some(Self::En),
            "pt-BR" | "pt-br" => Some(Self::PtBr),
            _ => None,
        }
    }

    /// As stored on `accounts.locale`; admin-created accounts have none.
    pub fn from_db(stored: Option<&str>) -> Self {
        stored.and_then(Self::parse).unwrap_or(Self::En)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::PtBr => "pt-BR",
        }
    }
}

const DOWNLOAD_URL: &str = "https://havenkeys.net/download";
const PRICING_URL: &str = "https://havenkeys.net/pricing";

pub fn signup_code(locale: Locale, code: &str) -> (String, String) {
    match locale {
        Locale::En => (
            "Your HavenKeys sign-up code".into(),
            format!(
                "Your HavenKeys sign-up code is:\n\n{code}\n\n\
                 It is valid for 15 minutes. If you did not ask for it, ignore this message; \
                 nothing happens without the code.\n"
            ),
        ),
        Locale::PtBr => (
            "Seu código de cadastro no HavenKeys".into(),
            format!(
                "Seu código de cadastro no HavenKeys é:\n\n{code}\n\n\
                 Ele vale por 15 minutos. Se você não pediu este código, ignore esta mensagem; \
                 nada acontece sem ele.\n"
            ),
        ),
    }
}

pub fn signup_invite(locale: Locale, invite: &str) -> (String, String) {
    match locale {
        Locale::En => (
            "Finish setting up HavenKeys".into(),
            format!(
                "Your HavenKeys account is ready to be set up.\n\n\
                 Install HavenKeys from {DOWNLOAD_URL}, open it, choose \"I have a setup code\" \
                 and paste this code:\n\n{invite}\n\n\
                 It is valid for 24 hours and works once. Whoever has this code and this mailbox \
                 can set up the account, so do not forward this message.\n"
            ),
        ),
        Locale::PtBr => (
            "Conclua a configuração do HavenKeys".into(),
            format!(
                "Sua conta HavenKeys está pronta para ser configurada.\n\n\
                 Instale o HavenKeys em {DOWNLOAD_URL}, abra o app, escolha \"Tenho um código de \
                 configuração\" e cole este código:\n\n{invite}\n\n\
                 Ele vale por 24 horas e funciona uma única vez. Quem tiver este código e esta \
                 caixa de e-mail consegue configurar a conta, então não encaminhe esta mensagem.\n"
            ),
        ),
    }
}

pub fn already_registered(locale: Locale) -> (String, String) {
    match locale {
        Locale::En => (
            "You already have a HavenKeys account".into(),
            "Someone asked to create a HavenKeys account with this address, but it already has \
             one.\n\nSign in from the HavenKeys app with your email, master password and Secret \
             Key. If this was not you, nothing has changed and you can ignore this message.\n"
                .into(),
        ),
        Locale::PtBr => (
            "Você já tem uma conta HavenKeys".into(),
            "Alguém pediu para criar uma conta HavenKeys com este endereço, mas ele já tem \
             uma.\n\nEntre pelo app HavenKeys com seu e-mail, senha mestra e Secret Key. Se não \
             foi você, nada mudou e você pode ignorar esta mensagem.\n"
                .into(),
        ),
    }
}

pub fn trial_ending(locale: Locale, days: i64) -> (String, String) {
    match locale {
        Locale::En => (
            format!("Your HavenKeys trial ends in {days} days"),
            format!(
                "Your free trial of HavenKeys ends in {days} days.\n\n\
                 After that, your vault stays readable on every device and you can export it \
                 at any time, but changes and autofill stop until you subscribe.\n\n\
                 Plans: {PRICING_URL}\n"
            ),
        ),
        Locale::PtBr => (
            format!(
                "Seu período de teste do HavenKeys termina em {days} {}",
                if days == 1 { "dia" } else { "dias" }
            ),
            format!(
                "Seu período de teste gratuito do HavenKeys termina em {days} {}.\n\n\
                 Depois disso, seu cofre continua legível em todos os dispositivos e você pode \
                 exportá-lo quando quiser, mas alterações e preenchimento automático param até \
                 você assinar.\n\nPlanos: {PRICING_URL}\n",
                if days == 1 { "dia" } else { "dias" }
            ),
        ),
    }
}

pub fn trial_ended(locale: Locale) -> (String, String) {
    match locale {
        Locale::En => (
            "Your HavenKeys trial has ended".into(),
            format!(
                "Your free trial of HavenKeys has ended.\n\n\
                 Your vault is still readable on every device and you can export it at any time. \
                 Changes and autofill are paused until you subscribe.\n\nPlans: {PRICING_URL}\n"
            ),
        ),
        Locale::PtBr => (
            "Seu período de teste do HavenKeys terminou".into(),
            format!(
                "Seu período de teste gratuito do HavenKeys terminou.\n\n\
                 Seu cofre continua legível em todos os dispositivos e você pode exportá-lo \
                 quando quiser. Alterações e preenchimento automático ficam pausados até você \
                 assinar.\n\nPlanos: {PRICING_URL}\n"
            ),
        ),
    }
}
```

The English `trial_ending` subject reads "ends in 1 days" for one day; the job (Task 6) sends it once, three days out, so `days` is 3 or 2 in practice. Leave the English plural as is.

Add `pub mod mail;` to `lib.rs` (after `pub mod locate;`).

- [ ] **Step 6: Configuration**

Rewrite `crates/havenkeys-server/src/config.rs` so `from_env` reads through a testable `from_vars`:

```rust
pub struct SmtpConfig {
    pub url: Zeroizing<String>,
    pub from: String,
}

pub struct Config {
    pub database_url: Zeroizing<String>,
    pub server_secret: [u8; 32],
    pub port: u16,
    pub cors_origin: Option<String>,
    pub trust_forwarded_for: bool,
    pub geoip_database: Option<std::path::PathBuf>,
    /// `HAVENKEYS_SIGNUP=open`. Off by default: a self-hosted server keeps
    /// admin invites only (spec 2026-10-07 §4.1).
    pub signup_open: bool,
    /// `HAVENKEYS_PUBLIC_URL`, carried in the invites signup issues.
    pub public_url: Option<String>,
    /// `SMTP_URL` and `SMTP_FROM`. Required when signup is open; useful
    /// without it for the trial notices of `admin new-account --trial`.
    pub smtp: Option<SmtpConfig>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let vars: Vec<(&str, String)> = [
            "DATABASE_URL",
            "SERVER_SECRET",
            "PORT",
            "HAVENKEYS_CORS_ORIGIN",
            "HAVENKEYS_TRUST_FORWARDED_FOR",
            "HAVENKEYS_GEOIP_DATABASE",
            "HAVENKEYS_SIGNUP",
            "HAVENKEYS_PUBLIC_URL",
            "SMTP_URL",
            "SMTP_FROM",
        ]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok().map(|v| (name, v)))
        .collect();
        Self::from_vars(&vars)
    }

    /// Reads a set of variables. Fails loudly rather than inventing a
    /// default secret: a predictable `SERVER_SECRET` would make the
    /// `auth/params` salts guessable and bring account enumeration back.
    pub fn from_vars(vars: &[(&str, String)]) -> Result<Self, String> {
        let get = |name: &str| -> Option<String> {
            vars.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let database_url =
            Zeroizing::new(get("DATABASE_URL").ok_or("DATABASE_URL is not set".to_string())?);
        let raw = Zeroizing::new(
            get("SERVER_SECRET")
                .ok_or("SERVER_SECRET is not set (32 random bytes, base64)".to_string())?,
        );
        let decoded = Zeroizing::new(
            BASE64
                .decode(raw.as_bytes())
                .map_err(|_| "SERVER_SECRET is not valid base64".to_string())?,
        );
        let server_secret: [u8; 32] = decoded
            .as_slice()
            .try_into()
            .map_err(|_| "SERVER_SECRET must decode to exactly 32 bytes".to_string())?;
        let port = match get("PORT") {
            Some(p) => p.parse().map_err(|_| "PORT is not a number".to_string())?,
            None => 8080,
        };
        let signup_open = match get("HAVENKEYS_SIGNUP").as_deref() {
            None | Some("off") => false,
            Some("open") => true,
            Some(_) => return Err("HAVENKEYS_SIGNUP must be open or off".into()),
        };
        let smtp = match (get("SMTP_URL"), get("SMTP_FROM")) {
            (Some(url), Some(from)) => Some(SmtpConfig {
                url: Zeroizing::new(url),
                from,
            }),
            (None, None) => None,
            _ => return Err("SMTP_URL and SMTP_FROM must be set together".into()),
        };
        let public_url = get("HAVENKEYS_PUBLIC_URL")
            .map(|u| check_public_url(&u))
            .transpose()?;
        if signup_open && smtp.is_none() {
            return Err("HAVENKEYS_SIGNUP=open needs SMTP_URL and SMTP_FROM".into());
        }
        if signup_open && public_url.is_none() {
            return Err("HAVENKEYS_SIGNUP=open needs HAVENKEYS_PUBLIC_URL".into());
        }
        Ok(Self {
            database_url,
            server_secret,
            port,
            cors_origin: get("HAVENKEYS_CORS_ORIGIN"),
            trust_forwarded_for: matches!(
                get("HAVENKEYS_TRUST_FORWARDED_FOR").as_deref(),
                Some("1") | Some("true")
            ),
            geoip_database: get("HAVENKEYS_GEOIP_DATABASE").map(Into::into),
            signup_open,
            public_url,
            smtp,
        })
    }
}
```

Keep `check_public_url` and the existing tests in this file. `data_encoding` is already a dependency.

- [ ] **Step 7: State and startup**

In `crates/havenkeys-server/src/routes/mod.rs`, add to `AppState`:

```rust
    /// Sends the signup and trial emails; none when SMTP is not configured.
    pub mailer: Option<std::sync::Arc<dyn crate::mail::Mailer>>,
    /// The server's public URL when signup is open (`HAVENKEYS_SIGNUP=open`),
    /// carried in the invites it issues; `None` answers the signup routes
    /// with 404.
    pub signup_url: Option<String>,
```

In `crates/havenkeys-server/src/main.rs`, `serve`, build the mailer before the state:

```rust
    let mailer: Option<std::sync::Arc<dyn havenkeys_server::mail::Mailer>> = match &config.smtp {
        Some(smtp) => match havenkeys_server::mail::Smtp::new(&smtp.url, &smtp.from) {
            Ok(sender) => Some(std::sync::Arc::new(sender)),
            Err(message) => {
                eprintln!("configuration error: {message}");
                return std::process::ExitCode::FAILURE;
            }
        },
        None => None,
    };
    let state = AppState {
        pool,
        server_secret: config.server_secret,
        trust_forwarded_for: config.trust_forwarded_for,
        cors_origin: config.cors_origin.clone(),
        locator: /* unchanged */,
        max_vault_bytes: havenkeys_server::limits::MAX_VAULT_BYTES,
        mailer,
        signup_url: if config.signup_open { config.public_url.clone() } else { None },
    };
```

In `crates/havenkeys-server/tests/support/mod.rs`, replace `start_with_vault_limit` with a general constructor and keep the old names:

```rust
use havenkeys_server::mail::Recording;
use std::sync::Arc;

pub struct Options {
    pub max_vault_bytes: i64,
    pub signup: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_vault_bytes: havenkeys_server::limits::MAX_VAULT_BYTES,
            signup: false,
        }
    }
}

impl TestServer {
    pub async fn start() -> Self {
        Self::start_with(Options::default()).await.0
    }

    pub async fn start_with_vault_limit(max_vault_bytes: i64) -> Self {
        Self::start_with(Options {
            max_vault_bytes,
            ..Options::default()
        })
        .await
        .0
    }

    /// Signup open with the public URL `https://vault.example.com`; every
    /// mail lands in the returned recorder.
    pub async fn start_signup() -> (Self, Arc<Recording>) {
        Self::start_with(Options {
            signup: true,
            ..Options::default()
        })
        .await
    }

    pub async fn start_with(opts: Options) -> (Self, Arc<Recording>) {
        /* the body of the old start_with_vault_limit, with: */
        let mailer = Arc::new(Recording::default());
        let state = AppState {
            pool: pool.clone(),
            server_secret: [7u8; 32],
            trust_forwarded_for: false,
            cors_origin: None,
            locator: None,
            max_vault_bytes: opts.max_vault_bytes,
            mailer: Some(mailer.clone()),
            signup_url: opts.signup.then(|| "https://vault.example.com".to_string()),
        };
        /* ... */
        (Self { /* as before */ }, mailer)
    }
}
```

- [ ] **Step 8: Run everything**

Run: `cargo test -p havenkeys-server`
Expected: all PASS (3 template tests, the config test, the moved URL test, every integration test unchanged).

- [ ] **Step 9: Commit**

```bash
git add crates/havenkeys-server Cargo.lock
git commit -m "feat(server): Mailer trait with SMTP over lettre, the five emails, signup and SMTP configuration"
```

---

### Task 5: The signup routes

**Files:**
- Modify: `crates/havenkeys-server/src/auth/rate_limit.rs` (`charge_window`, `over_window`)
- Create: `crates/havenkeys-server/src/routes/signup.rs`
- Modify: `crates/havenkeys-server/src/routes/mod.rs` (`pub mod signup;`, two routes)
- Modify: `crates/havenkeys-server/src/invite.rs` (`SIGNUP_INVITE_TTL_HOURS`)
- Modify: `crates/havenkeys-server/tests/support/mod.rs` (signup helpers)
- Test: `crates/havenkeys-server/tests/signup.rs` (new)

**Interfaces:**
- Consumes: `AppState.mailer`, `AppState.signup_url`, `mail::templates`, `billing::{set_status, load, Actor, Status}`, `invite::{generate_secret, hash, encode, Invite}`, `routes::auth::client_ip`, `email::normalize`.
- Produces:
  - `POST /v1/signup/start { email, locale, acceptedTerms }` → `202 {}`; `400 invalid_request`; `404` when off; `429`; `503` when mail fails.
  - `POST /v1/signup/verify { email, code }` → `200 { "invite": "HKINV1-…" }`; `400 invalid_request "code is not valid"`; `404`; `429`.
  - `rate_limit::charge_window(db, key, max, window_minutes) -> Result<(), ApiError>` and `rate_limit::over_window(db, key, max, window_minutes) -> Result<(), ApiError>`.
  - `signup::code_hash(secret, code) -> Vec<u8>` (pub, for the tests).
  - `invite::SIGNUP_INVITE_TTL_HOURS = 24`.
  - Test helpers `support::{start_signup_for, verify_signup, code_in, invite_in}`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/havenkeys-server/tests/support/mod.rs`:

```rust
pub async fn start_signup_for(server: &TestServer, email: &str) -> (u16, Value) {
    let res = server
        .post("/v1/signup/start")
        .json(&json!({ "email": email, "locale": "en", "acceptedTerms": "2026-10-20" }))
        .send()
        .await
        .unwrap();
    let status = res.status().as_u16();
    let text = res.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

pub async fn verify_signup(server: &TestServer, email: &str, code: &str) -> (u16, Value) {
    let res = server
        .post("/v1/signup/verify")
        .json(&json!({ "email": email, "code": code }))
        .send()
        .await
        .unwrap();
    let status = res.status().as_u16();
    let text = res.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

/// The six-digit code in a mail body: the only word of six digits.
pub fn code_in(mail: &havenkeys_server::mail::Mail) -> Option<String> {
    mail.body
        .split_whitespace()
        .find(|w| w.len() == 6 && w.bytes().all(|b| b.is_ascii_digit()))
        .map(str::to_string)
}

pub fn invite_in(mail: &havenkeys_server::mail::Mail) -> Option<String> {
    mail.body
        .split_whitespace()
        .find(|w| w.starts_with("HKINV1-"))
        .map(str::to_string)
}

/// Start and verify, returning the invite. The recorder's last mail is the
/// invite mail afterwards.
pub async fn signup(server: &TestServer, mailer: &Recording, email: &str) -> String {
    let (status, _) = start_signup_for(server, email).await;
    assert_eq!(status, 202);
    let code = code_in(mailer.sent().last().unwrap()).expect("a code was mailed");
    let (status, body) = verify_signup(server, email, &code).await;
    assert_eq!(status, 200, "{body}");
    body["invite"].as_str().unwrap().to_string()
}
```

Create `crates/havenkeys-server/tests/signup.rs`:

```rust
mod support;

use havenkeys_server::invite;
use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::{Digest, Sha256};
use support::*;
use uuid::Uuid;

#[tokio::test]
async fn signup_routes_answer_404_when_signup_is_off() {
    let server = TestServer::start().await;
    let (status, _) = start_signup_for(&server, "new@example.com").await;
    assert_eq!(status, 404);
    let (status, _) = verify_signup(&server, "new@example.com", "123456").await;
    assert_eq!(status, 404);
    server.cleanup().await;
}

#[tokio::test]
async fn start_answers_the_same_for_a_new_and_an_existing_email() {
    let (server, mailer) = TestServer::start_signup().await;
    signed_in(&server, "old@example.com").await;
    let (s1, b1) = start_signup_for(&server, "old@example.com").await;
    let (s2, b2) = start_signup_for(&server, "new@example.com").await;
    assert_eq!((s1, &b1), (202, &b2));
    let sent = mailer.sent();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].to, "old@example.com");
    assert!(code_in(&sent[0]).is_none(), "an existing account gets no code");
    assert!(sent[0].subject.contains("already have"));
    assert!(code_in(&sent[1]).is_some());
    let codes: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(codes, 1, "no code row for an existing account");
    server.cleanup().await;
}

#[tokio::test]
async fn a_right_code_creates_an_invited_trial_account_and_mails_the_invite() {
    let (server, mailer) = TestServer::start_signup().await;
    let invite_str = signup(&server, &mailer, "New@Example.com").await;
    let parsed = invite::decode(&invite_str).unwrap();
    assert_eq!(parsed.email, "new@example.com");
    assert_eq!(parsed.server, "https://vault.example.com");
    assert_eq!(invite_in(mailer.sent().last().unwrap()).as_deref(), Some(invite_str.as_str()));

    let db = server.db().await;
    let row = db
        .query_one(
            "SELECT a.status, a.created_by, a.terms_version, a.terms_accepted_at IS NOT NULL,
                    a.locale, p.status, p.trial_ends_at,
                    extract(epoch FROM a.invite_expires_at - now()) / 3600
               FROM accounts a JOIN subscriptions p ON p.account_id = a.id WHERE a.id = $1",
            &[&parsed.account],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "invited");
    assert_eq!(row.get::<_, String>(1), "signup");
    assert_eq!(row.get::<_, Option<String>>(2).as_deref(), Some("2026-10-20"));
    assert!(row.get::<_, bool>(3));
    assert_eq!(row.get::<_, Option<String>>(4).as_deref(), Some("en"));
    assert_eq!(row.get::<_, String>(5), "trialing");
    assert!(row.get::<_, Option<chrono::DateTime<chrono::Utc>>>(6).is_none());
    let hours: f64 = row.get(7);
    assert!((23.9..=24.0).contains(&hours), "a 24 h invite, not the admin's 7 days: {hours}");
    let codes: i64 = db
        .query_one("SELECT count(*) FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(codes, 0, "a spent code is deleted");
    drop(db);

    // The invite activates like any other, and the trial starts then.
    let vault_id = Uuid::new_v4();
    let res = server
        .post("/v1/accounts/activate")
        .json(&activate_body("new@example.com", &invite_str, vault_id, &[5u8; 32]))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let account = Account {
        email: "new@example.com".into(),
        account_id: parsed.account,
        vault_id,
        auth_key: [5u8; 32],
    };
    let sess = login(&server, &account, "Desktop").await;
    let pulled = pull(&server, &sess, 0).await;
    assert_eq!(pulled["account"]["status"], "trialing");
    assert_eq!(pulled["account"]["entitlement"], "full");
    assert!(pulled["account"]["trialEndsAt"].is_string());
    server.cleanup().await;
}

#[tokio::test]
async fn verify_accepts_the_email_in_any_spelling() {
    let (server, mailer) = TestServer::start_signup().await;
    let (status, _) = start_signup_for(&server, "  Mixed.Case@Example.COM ").await;
    assert_eq!(status, 202);
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    let (status, _) = verify_signup(&server, "mixed.case@example.com", &code).await;
    assert_eq!(status, 200);
    server.cleanup().await;
}

#[tokio::test]
async fn five_wrong_codes_kill_the_code() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    let wrong = if code == "000000" { "000001" } else { "000000" };
    for _ in 0..4 {
        let (status, body) = verify_signup(&server, "new@example.com", wrong).await;
        assert_eq!(status, 400);
        assert_eq!(body["error"]["code"], "invalid_request");
    }
    // The fifth wrong attempt is the last the code survives.
    let (status, _) = verify_signup(&server, "new@example.com", wrong).await;
    assert_eq!(status, 400);
    let (status, _) = verify_signup(&server, "new@example.com", &code).await;
    assert_eq!(status, 400, "dead after five wrong attempts");
    let codes: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(codes, 0);
    server.cleanup().await;
}

#[tokio::test]
async fn an_expired_code_is_refused() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    server
        .db()
        .await
        .execute("UPDATE signup_codes SET expires_at = now() - interval '1 second'", &[])
        .await
        .unwrap();
    let (status, _) = verify_signup(&server, "new@example.com", &code).await;
    assert_eq!(status, 400);
    server.cleanup().await;
}

#[tokio::test]
async fn the_stored_code_hash_is_keyed_by_the_server_secret() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    let stored: Vec<u8> = server
        .db()
        .await
        .query_one("SELECT code_hash FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_ne!(stored, Sha256::digest(code.as_bytes()).to_vec(), "not a bare hash");
    let mut mac = Hmac::<Sha256>::new_from_slice(&[7u8; 32]).unwrap();
    mac.update(code.as_bytes());
    assert_eq!(stored, mac.finalize().into_bytes().to_vec());
    assert_eq!(
        stored,
        havenkeys_server::routes::signup::code_hash(&[7u8; 32], &code)
    );
    server.cleanup().await;
}

#[tokio::test]
async fn a_second_start_replaces_the_code() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let first = code_in(&mailer.sent()[0]).unwrap();
    // Burn an attempt, then resend: the new code starts with zero attempts.
    let wrong = if first == "000000" { "000001" } else { "000000" };
    verify_signup(&server, "new@example.com", wrong).await;
    start_signup_for(&server, "new@example.com").await;
    let second = code_in(&mailer.sent()[1]).unwrap();
    let attempts: i32 = server
        .db()
        .await
        .query_one("SELECT attempts FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(attempts, 0);
    if first != second {
        let (status, _) = verify_signup(&server, "new@example.com", &first).await;
        assert_eq!(status, 400, "the old code is dead");
    }
    let (status, _) = verify_signup(&server, "new@example.com", &second).await;
    assert_eq!(status, 200);
    server.cleanup().await;
}

#[tokio::test]
async fn starts_are_limited_per_email() {
    let (server, _) = TestServer::start_signup().await;
    for _ in 0..3 {
        let (status, _) = start_signup_for(&server, "same@example.com").await;
        assert_eq!(status, 202);
    }
    let (status, body) = start_signup_for(&server, "same@example.com").await;
    assert_eq!(status, 429, "{body}");
    assert_eq!(body["error"]["code"], "rate_limited");
    // The address counter is separate: another email is still fine (4 of 5).
    let (status, _) = start_signup_for(&server, "other@example.com").await;
    assert_eq!(status, 202);
    server.cleanup().await;
}

#[tokio::test]
async fn starts_are_limited_per_address() {
    let (server, _) = TestServer::start_signup().await;
    for n in 0..5 {
        let (status, _) = start_signup_for(&server, &format!("u{n}@example.com")).await;
        assert_eq!(status, 202);
    }
    let (status, _) = start_signup_for(&server, "u6@example.com").await;
    assert_eq!(status, 429);
    server.cleanup().await;
}

#[tokio::test]
async fn verify_failures_count_against_the_address() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    let wrong = if code == "000000" { "000001" } else { "000000" };
    // One start plus nine failures fill the address window (10). The code
    // itself died at five; the later failures hit "no code" and still count.
    for _ in 0..9 {
        let (status, _) = verify_signup(&server, "new@example.com", wrong).await;
        assert_eq!(status, 400);
    }
    let (status, _) = verify_signup(&server, "nobody@example.com", "123456").await;
    assert_eq!(status, 429);
    server.cleanup().await;
}

#[tokio::test]
async fn two_verify_calls_racing_on_one_code_one_wins() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    let (a, b) = tokio::join!(
        verify_signup(&server, "new@example.com", &code),
        verify_signup(&server, "new@example.com", &code)
    );
    let mut statuses = [a.0, b.0];
    statuses.sort();
    assert_eq!(statuses, [200, 400]);
    let accounts: i64 = server
        .db()
        .await
        .query_one("SELECT count(*) FROM accounts", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(accounts, 1);
    server.cleanup().await;
}

#[tokio::test]
async fn an_admin_invited_account_gets_its_invite_replaced_by_signup() {
    let (server, mailer) = TestServer::start_signup().await;
    let old = new_invite(&server, "both@example.com").await;
    let new = signup(&server, &mailer, "both@example.com").await;
    assert_eq!(invite::decode(&old).unwrap().account, invite::decode(&new).unwrap().account);
    let res = server
        .post("/v1/accounts/activate")
        .json(&activate_body("both@example.com", &old, Uuid::new_v4(), &[5u8; 32]))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "the old invite stopped working");
    let res = server
        .post("/v1/accounts/activate")
        .json(&activate_body("both@example.com", &new, Uuid::new_v4(), &[5u8; 32]))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let plan: String = server
        .db()
        .await
        .query_one(
            "SELECT p.status FROM subscriptions p JOIN accounts a ON a.id = p.account_id WHERE a.email_normalized = $1",
            &[&"both@example.com"],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(plan, "complimentary", "the admin's plan row is kept");
    server.cleanup().await;
}

#[tokio::test]
async fn start_answers_503_uniformly_when_mail_fails() {
    let (server, mailer) = TestServer::start_signup().await;
    signed_in(&server, "old@example.com").await;
    mailer.fail.store(true, std::sync::atomic::Ordering::SeqCst);
    let (s1, b1) = start_signup_for(&server, "old@example.com").await;
    let (s2, b2) = start_signup_for(&server, "new@example.com").await;
    assert_eq!((s1, &b1), (503, &b2));
    server.cleanup().await;
}

#[tokio::test]
async fn verify_still_returns_the_invite_when_mail_fails() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "new@example.com").await;
    let code = code_in(mailer.sent().last().unwrap()).unwrap();
    mailer.fail.store(true, std::sync::atomic::Ordering::SeqCst);
    let (status, body) = verify_signup(&server, "new@example.com", &code).await;
    assert_eq!(status, 200);
    assert!(body["invite"].as_str().unwrap().starts_with("HKINV1-"));
    server.cleanup().await;
}

#[tokio::test]
async fn malformed_signup_requests_are_refused() {
    let (server, _) = TestServer::start_signup().await;
    for body in [
        json!({ "email": "a@example.com", "locale": "fr", "acceptedTerms": "2026-10-20" }),
        json!({ "email": "a@example.com", "locale": "en", "acceptedTerms": "yes" }),
        json!({ "email": "a@example.com", "locale": "en", "acceptedTerms": "" }),
        json!({ "email": "not-an-email", "locale": "en", "acceptedTerms": "2026-10-20" }),
        json!({ "email": "a@example.com", "locale": "en" }),
        json!({ "email": "a@example.com", "locale": "en", "acceptedTerms": "2026-10-20", "extra": 1 }),
    ] {
        let status = server
            .post("/v1/signup/start")
            .json(&body)
            .send()
            .await
            .unwrap()
            .status();
        assert_eq!(status, 400, "{body}");
    }
    for code in ["12345", "1234567", "12345a", ""] {
        let (status, _) = verify_signup(&server, "a@example.com", code).await;
        assert_eq!(status, 400, "{code:?}");
    }
    server.cleanup().await;
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p havenkeys-server --test signup`
Expected: compile error (`routes::signup` missing); after stubbing, every test but the `404` one fails.

- [ ] **Step 3: The window counter**

Append to `crates/havenkeys-server/src/auth/rate_limit.rs`:

```rust
/// Count one event against `key` and refuse once more than `max` happened
/// in the last `window_minutes`. For the signup routes, where every call
/// costs an email or a code check and no right answer clears the counter
/// (spec 2026-10-07 §4.4). Rows older than the window are reused, and the
/// daily task deletes stale `signup-*` rows.
pub async fn charge_window(
    db: &impl deadpool_postgres::GenericClient,
    key: &str,
    max: i32,
    window_minutes: i32,
) -> Result<(), ApiError> {
    let count: i32 = db
        .query_one(
            "INSERT INTO login_attempts (key, failures, window_start)
             VALUES ($1, 1, now())
             ON CONFLICT (key) DO UPDATE SET
               failures = CASE
                 WHEN login_attempts.window_start < now() - make_interval(mins => $2)
                 THEN 1 ELSE login_attempts.failures + 1 END,
               window_start = CASE
                 WHEN login_attempts.window_start < now() - make_interval(mins => $2)
                 THEN now() ELSE login_attempts.window_start END
             RETURNING failures",
            &[&key, &window_minutes],
        )
        .await?
        .get(0);
    if count > max {
        return Err(ApiError::RateLimited);
    }
    Ok(())
}

/// Refuse while `max` or more events are already in the window, without
/// counting this one. `verify` checks before the code is compared and
/// charges only a failure.
pub async fn over_window(
    db: &impl deadpool_postgres::GenericClient,
    key: &str,
    max: i32,
    window_minutes: i32,
) -> Result<(), ApiError> {
    let over = db
        .query_opt(
            "SELECT 1 FROM login_attempts
              WHERE key = $1
                AND failures >= $2
                AND window_start >= now() - make_interval(mins => $3)",
            &[&key, &max, &window_minutes],
        )
        .await?
        .is_some();
    if over {
        return Err(ApiError::RateLimited);
    }
    Ok(())
}
```

In `crates/havenkeys-server/src/invite.rs`, after `INVITE_TTL_DAYS`:

```rust
/// An invite issued by self-service signup (spec 2026-10-07 §4.2): the
/// user is at the keyboard, so a day is plenty.
pub const SIGNUP_INVITE_TTL_HOURS: i64 = 24;
```

- [ ] **Step 4: The routes**

Create `crates/havenkeys-server/src/routes/signup.rs`:

```rust
//! Self-service signup (spec 2026-10-07 §4): verify an email with a code,
//! then issue the same invite the admin CLI would.
//!
//! Neither route may reveal whether an email has an account. `start`
//! answers `202` with an empty body whether it mailed a code or a "you
//! already have an account" notice, and `verify` answers every failure
//! with one message. A code is six digits with five attempts and fifteen
//! minutes, so a guess has a 1 in 200,000 chance per code; it is stored
//! keyed by the server secret, so a dumped table cannot be brute-forced
//! offline. Whoever reads the mailbox can create the account, which is the
//! same trust the invite mail already places in it.

use crate::auth::rate_limit;
use crate::billing::{self, Actor, Status};
use crate::error::ApiError;
use crate::invite::{self, Invite, SIGNUP_INVITE_TTL_HOURS};
use crate::json::Json;
use crate::mail::templates::{self, Locale};
use crate::mail::Mail;
use crate::routes::auth::client_ip;
use crate::routes::AppState;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use rand::Rng;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use subtle::ConstantTimeEq;
use uuid::Uuid;
use zeroize::Zeroizing;

pub const CODE_TTL_MINUTES: i32 = 15;
pub const CODE_MAX_ATTEMPTS: i32 = 5;
pub const STARTS_PER_IP_PER_HOUR: i32 = 5;
pub const STARTS_PER_EMAIL_PER_HOUR: i32 = 3;
/// Shares the address key with `start` (spec §4.4), so the threshold must
/// leave room for one start plus a code's five attempts.
pub const VERIFY_FAILURES_PER_IP_PER_HOUR: i32 = 10;
const WINDOW_MINUTES: i32 = 60;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartRequest {
    email: String,
    locale: String,
    /// The terms version the user accepted: the terms page's date,
    /// `YYYY-MM-DD`. Recorded on the account (LGPD consent).
    accepted_terms: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifyRequest {
    email: String,
    code: String,
}

/// Six digits from the CSPRNG, zero-padded. `gen_range` is unbiased.
fn generate_code() -> Zeroizing<String> {
    let n: u32 = rand::thread_rng().gen_range(0..1_000_000);
    Zeroizing::new(format!("{n:06}"))
}

/// `HMAC-SHA-256(SERVER_SECRET, code)`: what the table holds.
pub fn code_hash(secret: &[u8; 32], code: &str) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("any key length is accepted");
    mac.update(code.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// The per-email counter's key: a hash, so the email is not written into
/// `login_attempts` in clear.
fn email_key(email: &str) -> String {
    let digest = Sha256::digest(email.as_bytes());
    format!("signup-email:{}", data_encoding::HEXLOWER.encode(&digest[..16]))
}

fn address_key(ip: &str) -> String {
    format!("signup-{}", rate_limit::ip_key(ip))
}

fn check_terms_version(raw: &str) -> Result<&str, ApiError> {
    const BAD: ApiError = ApiError::InvalidRequest("acceptedTerms is not valid");
    let b = raw.as_bytes();
    let shaped = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| matches!(i, 4 | 7) || c.is_ascii_digit());
    if shaped {
        Ok(raw)
    } else {
        Err(BAD)
    }
}

pub async fn start(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<StartRequest>,
) -> Result<(StatusCode, axum::Json<serde_json::Value>), ApiError> {
    // 404 unless signup is open: the route does not exist on a self-hosted
    // server (spec §4.1).
    let (Some(mailer), Some(_)) = (&state.mailer, &state.signup_url) else {
        return Err(ApiError::NotFound);
    };
    let locale = Locale::parse(&req.locale).ok_or(ApiError::InvalidRequest("locale is not valid"))?;
    let terms = check_terms_version(&req.accepted_terms)?;
    let email = crate::email::normalize(&req.email).map_err(ApiError::InvalidRequest)?;

    let db = state.pool.get().await?;
    let ip = client_ip(&state, &headers, peer);
    rate_limit::charge_window(&db, &address_key(&ip), STARTS_PER_IP_PER_HOUR, WINDOW_MINUTES).await?;
    rate_limit::charge_window(&db, &email_key(&email), STARTS_PER_EMAIL_PER_HOUR, WINDOW_MINUTES).await?;

    let existing: Option<String> = db
        .query_opt(
            "SELECT status FROM accounts WHERE email_normalized = $1",
            &[&email],
        )
        .await?
        .map(|r| r.get(0));
    let (subject, body) = match existing.as_deref() {
        // No account, or one still waiting for its invite: a code. The
        // upsert replaces a live code and resets its attempts.
        None | Some("invited") => {
            let code = generate_code();
            db.execute(
                "INSERT INTO signup_codes
                   (email_normalized, code_hash, locale, terms_version, expires_at, attempts, created_at)
                 VALUES ($1, $2, $3, $4, now() + make_interval(mins => $5), 0, now())
                 ON CONFLICT (email_normalized) DO UPDATE SET
                   code_hash = EXCLUDED.code_hash,
                   locale = EXCLUDED.locale,
                   terms_version = EXCLUDED.terms_version,
                   expires_at = EXCLUDED.expires_at,
                   attempts = 0,
                   created_at = now()",
                &[
                    &email,
                    &code_hash(&state.server_secret, &code),
                    &locale.as_str(),
                    &terms,
                    &CODE_TTL_MINUTES,
                ],
            )
            .await?;
            templates::signup_code(locale, &code)
        }
        Some(_) => templates::already_registered(locale),
    };
    // Both branches send one mail, so a failure answers 503 in both and a
    // success 202 in both: the outcome says nothing about the account.
    if mailer
        .send(Mail {
            to: email,
            subject,
            body,
        })
        .await
        .is_err()
    {
        return Err(ApiError::Unavailable);
    }
    tracing::info!(outcome = "started", "signup");
    Ok((StatusCode::ACCEPTED, axum::Json(serde_json::json!({}))))
}

pub async fn verify(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<VerifyRequest>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    const BAD: ApiError = ApiError::InvalidRequest("code is not valid");
    let (Some(mailer), Some(public_url)) = (&state.mailer, &state.signup_url) else {
        return Err(ApiError::NotFound);
    };
    let email = crate::email::normalize(&req.email).map_err(|_| BAD)?;
    if req.code.len() != 6 || !req.code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(BAD);
    }

    let mut db = state.pool.get().await?;
    let ip = address_key(&client_ip(&state, &headers, peer));
    rate_limit::over_window(&db, &ip, VERIFY_FAILURES_PER_IP_PER_HOUR, WINDOW_MINUTES).await?;

    let tx = db.transaction().await?;
    // The row lock serializes two verifies of one code: the second finds
    // the row gone (spent) or sees the attempt the first counted.
    let row = tx
        .query_opt(
            "SELECT code_hash, locale, terms_version, expires_at < now(), attempts
               FROM signup_codes WHERE email_normalized = $1 FOR UPDATE",
            &[&email],
        )
        .await?;
    let Some(row) = row else {
        drop(tx);
        rate_limit::charge_window(&db, &ip, VERIFY_FAILURES_PER_IP_PER_HOUR, WINDOW_MINUTES).await.ok();
        return Err(BAD);
    };
    let stored: Vec<u8> = row.get(0);
    let locale = Locale::from_db(Some(row.get::<_, String>(1).as_str()));
    let terms: String = row.get(2);
    let expired: bool = row.get(3);
    let attempts: i32 = row.get(4);
    let offered = code_hash(&state.server_secret, &req.code);
    let matches = stored.len() == offered.len() && stored.ct_eq(&offered).unwrap_u8() == 1;
    if expired || attempts >= CODE_MAX_ATTEMPTS || !matches {
        if expired || attempts + 1 >= CODE_MAX_ATTEMPTS {
            tx.execute(
                "DELETE FROM signup_codes WHERE email_normalized = $1",
                &[&email],
            )
            .await?;
        } else {
            tx.execute(
                "UPDATE signup_codes SET attempts = attempts + 1 WHERE email_normalized = $1",
                &[&email],
            )
            .await?;
        }
        tx.commit().await?;
        rate_limit::charge_window(&db, &ip, VERIFY_FAILURES_PER_IP_PER_HOUR, WINDOW_MINUTES).await.ok();
        tracing::info!(outcome = "rejected", "signup verify");
        return Err(BAD);
    }

    tx.execute(
        "DELETE FROM signup_codes WHERE email_normalized = $1",
        &[&email],
    )
    .await?;
    let secret = invite::generate_secret();
    let expires = Utc::now() + Duration::hours(SIGNUP_INVITE_TTL_HOURS);
    let existing = tx
        .query_opt(
            "SELECT id, status FROM accounts WHERE email_normalized = $1 FOR UPDATE",
            &[&email],
        )
        .await?;
    let account = match existing {
        None => {
            let id = Uuid::new_v4();
            tx.execute(
                "INSERT INTO accounts
                   (id, email_normalized, status, invite_hash, invite_expires_at, created_at,
                    created_by, terms_version, terms_accepted_at, locale)
                 VALUES ($1, $2, 'invited', $3, $4, now(), 'signup', $5, now(), $6)",
                &[
                    &id,
                    &email,
                    &invite::hash(&secret).to_vec(),
                    &expires,
                    &terms,
                    &locale.as_str(),
                ],
            )
            .await?;
            billing::set_status(&tx, id, Actor::System, Status::Trialing, None, "signup").await?;
            id
        }
        Some(row) if row.get::<_, String>(1) == "invited" => {
            // An invite already issued (by the admin, or by an earlier
            // signup) is replaced; the old one stops working. The plan row
            // is kept if there is one: the operator chose it.
            let id: Uuid = row.get(0);
            tx.execute(
                "UPDATE accounts
                    SET invite_hash = $2, invite_expires_at = $3, created_by = 'signup',
                        terms_version = $4, terms_accepted_at = now(), locale = $5
                  WHERE id = $1",
                &[&id, &invite::hash(&secret).to_vec(), &expires, &terms, &locale.as_str()],
            )
            .await?;
            if billing::load(&tx, id).await?.is_none() {
                billing::set_status(&tx, id, Actor::System, Status::Trialing, None, "signup").await?;
            }
            id
        }
        // Activated between start and verify: the code was for an account
        // that no longer needs one. Same answer as a wrong code.
        Some(_) => {
            tx.commit().await?;
            return Err(BAD);
        }
    };
    tx.commit().await?;

    let encoded = invite::encode(&Invite {
        server: public_url.clone(),
        email: email.clone(),
        account,
        secret: secret.to_string(),
    });
    // The page shows the invite, so a mail failure is not the user's
    // problem; it is logged by kind inside the mailer.
    let (subject, body) = templates::signup_invite(locale, &encoded);
    let _ = mailer
        .send(Mail {
            to: email,
            subject,
            body,
        })
        .await;
    tracing::info!(account_id = %account, outcome = "verified", "signup");
    Ok(axum::Json(serde_json::json!({ "invite": encoded })))
}
```

In `crates/havenkeys-server/src/routes/mod.rs`: add `pub mod signup;` and, after the `/v1/accounts/activate` route:

```rust
        .route(
            "/v1/signup/start",
            small(post(signup::start), MAX_ANONYMOUS_AUTH_BODY_BYTES),
        )
        .route(
            "/v1/signup/verify",
            small(post(signup::verify), MAX_ANONYMOUS_AUTH_BODY_BYTES),
        )
```

`data_encoding::HEXLOWER` exists in the already-used `data-encoding` crate.

- [ ] **Step 5: Run the signup tests**

Run: `cargo test -p havenkeys-server --test signup`
Expected: all 16 PASS.

- [ ] **Step 6: Run everything**

Run: `cargo test -p havenkeys-server && cargo clippy -p havenkeys-server --all-targets -- -D warnings`
Expected: all PASS, no warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): self-service signup with an emailed code behind HAVENKEYS_SIGNUP=open"
```

---

### Task 6: The daily task: sweeps and trial emails

**Files:**
- Create: `crates/havenkeys-server/src/billing/jobs.rs` (replace the placeholder)
- Modify: `crates/havenkeys-server/src/main.rs:91-111` (the daily loop)
- Test: `crates/havenkeys-server/tests/jobs.rs` (new)

**Interfaces:**
- Consumes: `erase::sweep_tombstones`, `billing::note`, `mail::{Mailer, Mail, templates}`, `routes::signup::ABANDONED_SIGNUP_DAYS` (add it: `pub const ABANDONED_SIGNUP_DAYS: i32 = 7;` in `signup.rs`).
- Produces: `billing::jobs::DailyReport { codes_swept, signups_abandoned, counters_swept, ending_notices, ended_notices }` and `billing::jobs::run_daily(db: &impl GenericClient, mailer: Option<&dyn Mailer>) -> Result<DailyReport, tokio_postgres::Error>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/havenkeys-server/tests/jobs.rs`:

```rust
mod support;

use havenkeys_server::billing::jobs::run_daily;
use havenkeys_server::billing::Status;
use support::*;
use uuid::Uuid;

#[tokio::test]
async fn abandoned_signups_are_swept_after_seven_days_and_nothing_else_is() {
    let (server, mailer) = TestServer::start_signup().await;
    // Three invited accounts: a signup from 8 days ago, a signup from
    // yesterday, and an admin invite from 8 days ago.
    signup(&server, &mailer, "stale@example.com").await;
    signup(&server, &mailer, "fresh@example.com").await;
    new_invite(&server, "admin@example.com").await;
    // And one signup that activated 8 days ago.
    let invite = signup(&server, &mailer, "active@example.com").await;
    let res = server
        .post("/v1/accounts/activate")
        .json(&activate_body("active@example.com", &invite, Uuid::new_v4(), &[5u8; 32]))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let db = server.db().await;
    db.execute(
        "UPDATE accounts SET created_at = now() - interval '8 days'
          WHERE email_normalized IN ('stale@example.com', 'admin@example.com', 'active@example.com')",
        &[],
    )
    .await
    .unwrap();

    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!(report.signups_abandoned, 1);
    let left: Vec<String> = db
        .query("SELECT email_normalized FROM accounts ORDER BY 1", &[])
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(left, ["active@example.com", "admin@example.com", "fresh@example.com"]);
    let orphans: i64 = db
        .query_one(
            "SELECT count(*) FROM subscriptions p LEFT JOIN accounts a ON a.id = p.account_id WHERE a.id IS NULL",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(orphans, 0);
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn expired_codes_and_stale_signup_counters_are_swept() {
    let (server, mailer) = TestServer::start_signup().await;
    start_signup_for(&server, "a@example.com").await;
    start_signup_for(&server, "b@example.com").await;
    let db = server.db().await;
    db.execute(
        "UPDATE signup_codes SET expires_at = now() - interval '1 minute' WHERE email_normalized = 'a@example.com'",
        &[],
    )
    .await
    .unwrap();
    db.execute(
        "UPDATE login_attempts SET window_start = now() - interval '3 hours' WHERE key LIKE 'signup-email:%'",
        &[],
    )
    .await
    .unwrap();
    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!(report.codes_swept, 1);
    assert_eq!(report.counters_swept, 2, "the two email counters; the address one is fresh");
    let codes: i64 = db
        .query_one("SELECT count(*) FROM signup_codes", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(codes, 1);
    let ip_rows: i64 = db
        .query_one("SELECT count(*) FROM login_attempts WHERE key LIKE 'signup-ip:%'", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(ip_rows, 1);
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn trial_notices_go_out_once_each() {
    let (server, mailer) = TestServer::start_signup().await;
    let (account, _) = signed_in(&server, "user@example.com").await;
    set_plan(&server, account.account_id, Status::Trialing).await;
    let db = server.db().await;
    db.execute(
        "UPDATE accounts SET locale = 'pt-BR' WHERE id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let before = mailer.sent().len();

    // Ten days left: nothing.
    db.execute(
        "UPDATE subscriptions SET trial_ends_at = now() + interval '10 days' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!((report.ending_notices, report.ended_notices), (0, 0));

    // Two and a half days left: the ending notice, once, in Portuguese.
    db.execute(
        "UPDATE subscriptions SET trial_ends_at = now() + interval '60 hours' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!(report.ending_notices, 1);
    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!(report.ending_notices, 0, "sent once");
    let sent = mailer.sent();
    assert_eq!(sent.len(), before + 1);
    assert_eq!(sent[before].to, "user@example.com");
    assert!(sent[before].subject.contains("termina em 3 dias"), "{}", sent[before].subject);

    // Ended: the ended notice, once.
    db.execute(
        "UPDATE subscriptions SET trial_ends_at = now() - interval '1 hour' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!(report.ended_notices, 1);
    let report = run_daily(&db, Some(mailer.as_ref())).await.unwrap();
    assert_eq!(report.ended_notices, 0);
    assert!(mailer.sent().last().unwrap().subject.contains("terminou"));

    let marks: i64 = db
        .query_one(
            "SELECT count(*) FROM billing_events WHERE account_id = $1 AND reason LIKE 'trial_%_notice'",
            &[&account.account_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(marks, 2);
    drop(db);
    server.cleanup().await;
}

#[tokio::test]
async fn without_a_mailer_no_notice_is_marked_as_sent() {
    let server = TestServer::start().await;
    let (account, _) = signed_in(&server, "user@example.com").await;
    set_plan(&server, account.account_id, Status::Trialing).await;
    let db = server.db().await;
    db.execute(
        "UPDATE subscriptions SET trial_ends_at = now() - interval '1 hour' WHERE account_id = $1",
        &[&account.account_id],
    )
    .await
    .unwrap();
    let report = run_daily(&db, None).await.unwrap();
    assert_eq!(report.ended_notices, 0);
    let marks: i64 = db
        .query_one(
            "SELECT count(*) FROM billing_events WHERE reason = 'trial_ended_notice'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(marks, 0, "unsent is unmarked, so it goes out once SMTP exists");
    drop(db);
    server.cleanup().await;
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p havenkeys-server --test jobs`
Expected: compile error (`run_daily` missing).

- [ ] **Step 3: Write the job**

Replace `crates/havenkeys-server/src/billing/jobs.rs`:

```rust
//! The daily task (spec 2026-10-07 §4.3, §4.5, §5.6): sweep what expired,
//! delete signups nobody finished, and send the two trial notices.
//!
//! Nothing here changes an entitlement; that is computed on every request.
//! A notice is marked in `billing_events` only after it was sent, so a day
//! without SMTP sends it the next day instead of never.

use super::note;
use crate::mail::templates::{self, Locale};
use crate::mail::{Mail, Mailer};
use crate::routes::signup::ABANDONED_SIGNUP_DAYS;
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use uuid::Uuid;

/// How many days before the end the "ending" notice goes out.
const ENDING_NOTICE_DAYS: i32 = 3;
const ENDING_REASON: &str = "trial_ending_notice";
const ENDED_REASON: &str = "trial_ended_notice";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DailyReport {
    pub codes_swept: u64,
    pub signups_abandoned: u64,
    pub counters_swept: u64,
    pub ending_notices: u64,
    pub ended_notices: u64,
}

pub async fn run_daily(
    db: &impl GenericClient,
    mailer: Option<&dyn Mailer>,
) -> Result<DailyReport, tokio_postgres::Error> {
    let mut report = DailyReport::default();
    report.codes_swept = db
        .execute("DELETE FROM signup_codes WHERE expires_at <= now()", &[])
        .await?;
    // A signup account that was never activated holds nothing but an
    // email; the cascade takes its plan row and its events.
    report.signups_abandoned = db
        .execute(
            "DELETE FROM accounts
              WHERE status = 'invited' AND created_by = 'signup'
                AND created_at < now() - make_interval(days => $1)",
            &[&ABANDONED_SIGNUP_DAYS],
        )
        .await?;
    report.counters_swept = db
        .execute(
            "DELETE FROM login_attempts
              WHERE key LIKE 'signup-%' AND window_start < now() - interval '2 hours'",
            &[],
        )
        .await?;
    let Some(mailer) = mailer else {
        return Ok(report);
    };

    let ending = db
        .query(
            "SELECT a.id, a.email_normalized, a.locale, p.trial_ends_at
               FROM subscriptions p JOIN accounts a ON a.id = p.account_id
              WHERE p.status = 'trialing' AND a.status = 'active'
                AND p.trial_ends_at > now()
                AND p.trial_ends_at <= now() + make_interval(days => $1)
                AND NOT EXISTS (SELECT 1 FROM billing_events e
                                 WHERE e.account_id = a.id AND e.reason = $2)",
            &[&ENDING_NOTICE_DAYS, &ENDING_REASON],
        )
        .await?;
    for row in &ending {
        let (id, email, locale, ends) = fields(row);
        let hours = (ends - Utc::now()).num_hours().max(0);
        let days = (hours + 23) / 24;
        let (subject, body) = templates::trial_ending(locale, days);
        if mailer.send(Mail { to: email, subject, body }).await.is_ok() {
            note(db, id, ENDING_REASON).await?;
            report.ending_notices += 1;
        }
    }

    let ended = db
        .query(
            "SELECT a.id, a.email_normalized, a.locale, p.trial_ends_at
               FROM subscriptions p JOIN accounts a ON a.id = p.account_id
              WHERE p.status = 'trialing' AND a.status = 'active'
                AND p.trial_ends_at <= now()
                AND NOT EXISTS (SELECT 1 FROM billing_events e
                                 WHERE e.account_id = a.id AND e.reason = $1)",
            &[&ENDED_REASON],
        )
        .await?;
    for row in &ended {
        let (id, email, locale, _) = fields(row);
        let (subject, body) = templates::trial_ended(locale);
        if mailer.send(Mail { to: email, subject, body }).await.is_ok() {
            note(db, id, ENDED_REASON).await?;
            report.ended_notices += 1;
        }
    }
    Ok(report)
}

fn fields(row: &tokio_postgres::Row) -> (Uuid, String, Locale, DateTime<Utc>) {
    let locale: Option<String> = row.get(2);
    (
        row.get(0),
        row.get(1),
        Locale::from_db(locale.as_deref()),
        row.get(3),
    )
}
```

Add to `crates/havenkeys-server/src/routes/signup.rs` constants: `pub const ABANDONED_SIGNUP_DAYS: i32 = 7;`.

- [ ] **Step 4: Wire the loop**

In `crates/havenkeys-server/src/main.rs`, replace the body of the spawned loop so each tick runs both jobs:

```rust
    // Once at start, then daily: expired tombstones of deleted accounts
    // (spec 2026-10-05 §4.5), expired signup codes, abandoned signups and
    // the trial notices (spec 2026-10-07 §4.5, §5.6). A failure is logged by
    // kind and the next day tries again.
    let sweeping = state.pool.clone();
    let notifying = state.mailer.clone();
    tokio::spawn(async move {
        let mut every = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
        loop {
            every.tick().await;
            let db = match sweeping.get().await {
                Ok(db) => db,
                Err(_) => {
                    tracing::warn!(kind = "pool", "database error");
                    continue;
                }
            };
            if havenkeys_server::erase::sweep_tombstones(&db).await.is_err() {
                tracing::warn!(kind = "sweep", "database error");
            }
            match havenkeys_server::billing::jobs::run_daily(&db, notifying.as_deref()).await {
                Ok(report) => tracing::info!(
                    codes = report.codes_swept,
                    signups = report.signups_abandoned,
                    ending = report.ending_notices,
                    ended = report.ended_notices,
                    "daily task"
                ),
                Err(_) => tracing::warn!(kind = "daily", "database error"),
            }
        }
    });
```

`notifying.as_deref()` turns `Option<Arc<dyn Mailer>>` into `Option<&dyn Mailer>`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p havenkeys-server --test jobs && cargo build -p havenkeys-server`
Expected: 4 PASS, the binary builds.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-server
git commit -m "feat(server): daily task sweeps codes, abandoned signups and counters, and sends the trial notices"
```

---

### Task 7: Erasure, the logging gate, documentation

**Files:**
- Modify: `crates/havenkeys-server/src/erase.rs:16-62`
- Modify: `crates/havenkeys-server/tests/deletion.rs`
- Modify: `crates/havenkeys-server/tests/no_logging.rs`
- Modify: `docs/deployment.md` (§2 environment table; a new §"Plans and signup")
- Modify: `docs/self-hosting.md`
- Modify: `docs/server-sync.md`
- Modify: `docs/security-model.md`
- Modify: `CLAUDE.md` (§1 amendments)
- Modify: `docs/ideas.md` (item 13)

- [ ] **Step 1: Write the failing tests**

Append to `crates/havenkeys-server/tests/deletion.rs`:

```rust
/// A code waiting for an invited signup account is the one place the email
/// would survive erasure; it must not.
#[tokio::test]
async fn erasing_an_account_takes_its_signup_code_and_billing_rows() {
    let (server, mailer) = TestServer::start_signup().await;
    support::signup(&server, &mailer, "gone@example.com").await;
    // A second start for the same still-invited address leaves a code row.
    support::start_signup_for(&server, "gone@example.com").await;
    admin::run(
        AdminCommand::DeleteAccount {
            email: "gone@example.com".into(),
        },
        server.pool(),
    )
    .await
    .unwrap();
    let db = server.db().await;
    for table in ["signup_codes", "subscriptions", "billing_events"] {
        let n: i64 = db
            .query_one(&format!("SELECT count(*) FROM {table}"), &[])
            .await
            .unwrap()
            .get(0);
        assert_eq!(n, 0, "{table}");
    }
    drop(db);
    assert_eq!(
        traces_of(&server, &["gone@example.com".to_string()]).await,
        Vec::<String>::new()
    );
    server.cleanup().await;
}
```

In `crates/havenkeys-server/tests/no_logging.rs`, `no_secret_reaches_a_log_line`: change the server start to `let (server, mailer) = support::TestServer::start_signup().await;`, and before the existing activation add a signup round trip whose values join the forbidden list:

```rust
    let signup_email = "signup-marker@example.com";
    let (status, _) = support::start_signup_for(&server, signup_email).await;
    assert_eq!(status, 202);
    let code = support::code_in(mailer.sent().last().unwrap()).unwrap();
    let (status, body) = support::verify_signup(&server, signup_email, &code).await;
    assert_eq!(status, 200);
    let signup_invite = body["invite"].as_str().unwrap().to_string();
```

and in the list of values the test asserts are absent from the captured log (the existing `for secret in [...]`-style check at the end), add `signup_email`, `code.as_str()`, `signup_invite.as_str()` and the SMTP-free marker `"signup-marker"`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p havenkeys-server --test deletion --test no_logging`
Expected: `erasing_an_account_takes_its_signup_code_and_billing_rows` FAILS on `signup_codes` (1 row left); `no_logging` PASSES already (nothing logs these values), which is fine: it is the regression gate.

- [ ] **Step 3: Erase the code row**

In `crates/havenkeys-server/src/erase.rs`, `erase_account`, before the final `DELETE FROM accounts`:

```rust
    // A signup code waiting for this address (the cascade cannot reach it:
    // codes are keyed by email, not account).
    tx.execute(
        "DELETE FROM signup_codes
          WHERE email_normalized = (SELECT email_normalized FROM accounts WHERE id = $1)",
        &[&account_id],
    )
    .await?;
```

Also delete the account's signup email counter is not possible (the key is a hash of the email and the sweep removes it within two hours), which the doc comment should say.

Run: `cargo test -p havenkeys-server --test deletion`
Expected: PASS.

- [ ] **Step 4: Documentation**

`docs/deployment.md`, §2 table, append rows:

```markdown
| `HAVENKEYS_SIGNUP` | no | `open` lets anyone create an account at `POST /v1/signup/start` after verifying their email with a code; `off` (the default) answers both signup routes with 404 and leaves `admin new-account` as the only way in. `open` requires the three variables below. |
| `HAVENKEYS_PUBLIC_URL` | with signup | The URL clients will talk to, carried in the invites signup issues, e.g. `https://api.havenkeys.net`. HTTPS, or `http://localhost` for development. |
| `SMTP_URL` | with signup | `smtps://user:pass@host:465` (implicit TLS) or `smtp://user:pass@host:587?tls=required` (STARTTLS). Plain SMTP is refused: the code in the mail is a credential. Without signup it is optional and only sends the trial notices for `admin new-account --trial`. |
| `SMTP_FROM` | with `SMTP_URL` | The sender, e.g. `HavenKeys <no-reply@havenkeys.net>`. |
```

and change the `HAVENKEYS_CORS_ORIGIN` row's text to: "Exactly one browser origin. Set it to the website's origin (`https://havenkeys.net` on the hosted server) when signup is open, because the signup page calls `/v1/signup/*` from the browser; otherwise leave it unset. There is no wildcard."

Add a section after the backups section:

```markdown
## Plans and signup

Every account has a plan row (`subscriptions`). Accounts that existed before
plans, and accounts from `admin new-account` without `--trial`, are
`complimentary` with no end date. `admin new-account --trial` and
self-service signup start a 14-day trial at activation.

When an account is frozen (trial over, `past_due` past its grace, or set
`frozen`), the server answers `402 account_frozen` on item writes, master
password changes, pairing approvals and logins from a device the account
never used. Read sync, logins from known devices, the header, the device
list, revoke, logout and account deletion keep working: the data is never
held hostage.

```sh
havenkeys-server admin set-plan --email you@example.com --status complimentary
havenkeys-server admin set-plan --email you@example.com --status active --until 2027-01-31
havenkeys-server admin set-plan --email you@example.com --status frozen
```

Every change lands in `billing_events` with who made it. A daily task
deletes expired signup codes, signups not activated within 7 days, and stale
signup rate-limit rows, and sends "your trial ends in 3 days" and "your
trial has ended" once each (only with `SMTP_URL`).

The signup code is six digits, valid 15 minutes, five attempts, stored as
`HMAC-SHA-256(SERVER_SECRET, code)`; `start` is limited to 5 per address and
3 per email per hour. Whoever reads the mailbox can create the account: that
is the same trust the invite email already places in it.
```

`docs/self-hosting.md`: in the step that creates the account, add one sentence: "Self-service signup is off on your server unless you set `HAVENKEYS_SIGNUP=open` (see deployment.md §Plans and signup); invites from the CLI are complimentary and never expire into a frozen account."

`docs/server-sync.md`: after the `POST /v1/account/delete` paragraph, add:

```markdown
### Plans

`POST /v1/auth/login` and `GET /v1/sync` carry
`"account": { "status", "entitlement", "trialEndsAt", "periodEnd" }`
(`status` one of `trialing`, `active`, `past_due`, `frozen`,
`complimentary`; `entitlement` `full` or `frozen`). A frozen account gets
`402 { "error": { "code": "account_frozen" } }` on `POST /v1/items`,
`POST /v1/account/credentials`, `POST /v1/pairings/{id}/approve` and a
`POST /v1/auth/login` from a device the account never used. Clients keep
the last `account` object they saw and decide locally what to disable;
the server guarantees only the refusals above
(spec `2026-10-07-self-signup-and-plans-design.md` §5.4, §6.4).

### Signup (`HAVENKEYS_SIGNUP=open` only)

`POST /v1/signup/start { email, locale, acceptedTerms }` → `202 {}` always
(a code is mailed, or a notice that the account exists; the answer does not
say which). `POST /v1/signup/verify { email, code }` → `200 { invite }`,
the same `HKINV1-…` string `admin new-account` prints, valid 24 h, also
mailed. Activation is unchanged.
```

`docs/security-model.md`: add under the server section:

```markdown
### Self-service signup and plans

- Email ownership is proved by a six-digit code sent over SMTP (15 minutes,
  five attempts, stored keyed by the server secret). The signup invite is
  then shown on the page and emailed. Anyone who controls the mailbox can
  create the account; the invite email already relied on that.
- The server answers `start` identically for new and existing addresses and
  rate limits it per address and per email, so signup is not an account
  enumeration oracle. A mail outage answers 503 for both.
- The entitlement is computed on every request from the plan row and the
  clock. A frozen account cannot write, change its password, approve a
  pairing or add a device. Everything that reads, exports, revokes or deletes
  keeps working.
- The freeze of reads and autofill inside the apps (spec §6.3) is enforced
  by open-source clients; a modified build can remove it. The server
  guarantees only the refusal of writes and of new devices.
- The SMTP provider sees the recipient address, the code and the invite. It
  is an operator under the LGPD and is named in the privacy policy.
```

`CLAUDE.md`, §1, after the 2026-10-05 account-deletion amendment, add verbatim from spec §3 with the date filled in:

```markdown
> Amended on 2026-10-09 by
> `docs/superpowers/specs/2026-10-07-self-signup-and-plans-design.md`: the
> hosted server accepts self-service signup (email verified with a code sent
> over SMTP) and accounts have a plan with a 14-day trial. When an account is
> frozen (trial over, or payment lapsed), the server refuses every write and
> new device; the apps stay readable and can export, but autofill stops: the
> extension's popup may only show or copy the current site's password and
> TOTP, and signing in with an existing passkey keeps working.
```

`docs/ideas.md`, item 13: change the status line to "Em andamento: plano em `docs/superpowers/plans/2026-10-09-self-signup-index.md`; etapa 1 (servidor) feita."

- [ ] **Step 5: Full verification**

Run:

```bash
cargo test -p havenkeys-server
cargo clippy -p havenkeys-server --all-targets -- -D warnings
cargo fmt --all -- --check
cargo deny check
cargo audit
```

Expected: all green. If `cargo deny` or `cargo audit` names a new advisory in lettre's tree, read it and record the decision in `docs/security-review.md` before continuing; do not add an ignore without a reason line.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-server docs CLAUDE.md
git commit -m "feat(server): erase signup codes with the account; document plans, signup and the new variables"
```

---

## Self-review notes

- Spec §4.1 (config), §4.2 (routes, uniform 202, replace invite, race), §4.3 (table, keyed hash, 5 attempts, sweep), §4.4 (limits), §4.5 (abandoned 7 days), §4.6 (logging) → Tasks 4, 5, 6, 7.
- Spec §5.1–§5.6 (tables, events, entitlement, enforcement list, `account` object, admin CLI, migration, provider seam, trial emails) → Tasks 1, 2, 3, 6.
- Spec §8 server bullets → every one has a named test above; `HAVENKEYS_SIGNUP=off` → `signup_routes_answer_404_when_signup_is_off`; migration → `plans_schema_is_constrained_and_existing_accounts_are_complimentary` (shape) plus the `INSERT … SELECT` in the migration, which the test databases exercise on an empty table only. The data move on a populated database is checked on the hosted server at deploy time: `admin list-accounts` must show `complimentary` for every account.
- Spec §3 CLAUDE.md amendment → Task 7.
- Not in this stage: `HAVENKEYS_PUBLIC_URL` is an addition the spec did not name; signup must know which server URL to put in the invite. The spec's "invite … 24 h" is `SIGNUP_INVITE_TTL_HOURS`; admin invites keep their 7 days.
