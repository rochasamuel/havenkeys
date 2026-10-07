# Self-service signup and plans — design

Date: 2026-10-07. Status: approved in conversation; **not scheduled**. The
owner decided not to implement it yet. Read the code before planning: the
routes, tables and files named here describe the repository as of 0.20.0.

## 1. Goal

Today an account on `api.havenkeys.net` exists only after the operator runs
`havenkeys-server admin new-account` and hands over an `HKINV1-…` invite.
This spec lets anyone create their own account, in a flow modelled on
1Password's, and lays down the data model and enforcement points needed to
charge for the hosted service later (trial, plan, subscription), without
integrating a payment gateway yet.

## 2. Decisions taken

| Question | Decision |
|---|---|
| Where the account is created | The website starts it (email, code, terms); the desktop or Android app finishes it (master password, Secret Key, Emergency Kit). No cryptography runs in the browser |
| How the site hands over to the app | The server issues an ordinary invite (`HKINV1-…`, 24 h) after the email is verified; the site shows it and it is also emailed. The app's existing activation path is reused |
| Which servers | Only a server with `HAVENKEYS_SIGNUP=open` (the hosted `api.havenkeys.net`). Self-hosted servers default to `off` and keep admin invites only |
| Email | Generic SMTP over TLS (`lettre`), configured by environment; no vendor API in the code |
| Trial | 14 days, no card, starting at activation |
| After the trial without payment | The account is **frozen**: read-only, autofill off (§6) |
| Existing accounts | Migrated to `complimentary` with no end date |
| Payment gateway | Out of scope; a `billing/` seam is prepared (§5.5) |
| Captcha | None at first (it would be a third-party API); Cloudflare Turnstile as an option if signup spam appears |
| Deep link `havenkeys://setup` | Later, as an improvement on top of the invite |

Approaches rejected:

- **Whole signup in the browser (as 1Password does)**: it needs the Rust core
  compiled to WebAssembly and puts key generation on a page served by Vercel,
  where served code can be swapped. A new attack surface and build path.
- **Whole signup in the app**: the simplest and safest, but the website could
  not capture the signup.
- **Site creates a pending account, app verifies the email again**: nothing
  sensitive travels by email, but it costs two verifications and new
  endpoints in both apps. Emailing the invite is acceptable because whoever
  reads that mailbox already controls the account it verified.

## 3. CLAUDE.md amendment

Under §1, after the 2026-10-05 account-deletion amendment, add:

> Amended on <date> by
> `docs/superpowers/specs/2026-10-07-self-signup-and-plans-design.md`: the
> hosted server accepts self-service signup (email verified with a code sent
> over SMTP) and accounts have a plan with a 14-day trial. When an account is
> frozen (trial over, or payment lapsed), the server refuses every write and
> new device; the apps stay readable and can export, but autofill stops: the
> extension's popup may only show or copy the current site's password and
> TOTP, and signing in with an existing passkey keeps working.

## 4. Signup on the server (`crates/havenkeys-server`)

### 4.1 Configuration

| Variable | Meaning |
|---|---|
| `HAVENKEYS_SIGNUP` | `open` or `off` (default `off`). With `off` the signup routes answer `404` |
| `SMTP_URL` | e.g. `smtps://user:pass@smtp.example.com:465`; required when signup is `open` |
| `SMTP_FROM` | Sender address |

`HAVENKEYS_CORS_ORIGIN` on the hosted server stays `https://havenkeys.net`.

### 4.2 Routes (unauthenticated)

```text
POST /v1/signup/start   { email, locale, acceptedTerms: "<terms version>" }
  → 202, always the same body.
    New email: sends a 6-digit code (15 min, 5 attempts).
    Email of an active account: sends "you already have an account,
    sign in from the app". The response never reveals which case it was.

POST /v1/signup/verify  { email, code }
  → 200 { invite: "HKINV1-…" }
```

On a matching code, `verify`, in one transaction:

1. creates the account with `status = 'invited'`, a 24 h invite and a
   `subscriptions` row in `trialing` with `trial_ends_at` NULL (the trial
   starts at activation, §5.1);
2. records `terms_version` and `terms_accepted_at` on the account (LGPD
   consent record);
3. deletes the code;
4. returns the invite and emails the same invite.

If the email already has a signup account still `invited`, its invite is
replaced (the old one stops working). Two `verify` calls racing on one code:
exactly one wins.

### 4.3 Table `signup_codes`

```text
email_normalized TEXT PRIMARY KEY
code_hash        BYTEA      -- HMAC-SHA-256(SERVER_SECRET, code): six digits
                            -- alone would fall to an offline brute force
expires_at       TIMESTAMPTZ
attempts         INTEGER
created_at       TIMESTAMPTZ
```

A wrong code increments `attempts`; at 5 the code is dead and a new `start`
is needed. A periodic sweep deletes expired rows.

### 4.4 Abuse limits

Reusing `login_attempts` keys: `signup-ip:<addr>` at 5 `start` per hour,
`signup-email:<hash>` at 3 per hour. `verify` failures count against the
same IP key.

### 4.5 Abandoned signups

An `invited` account created by signup and not activated within 7 days is
deleted with its subscription. No vault data exists at that point.

### 4.6 Logging

Email, code and invite are never logged, as in the existing routes.

## 5. Plans and entitlement

### 5.1 Table `subscriptions` (one row per account)

```text
account_id         UUID PRIMARY KEY REFERENCES accounts ON DELETE CASCADE
plan               TEXT NOT NULL                 -- 'personal'
status             TEXT NOT NULL CHECK (status IN
                     ('trialing','active','past_due','frozen','complimentary'))
trial_ends_at      TIMESTAMPTZ                   -- set at activation: now() + 14 days
current_period_end TIMESTAMPTZ                   -- paid until
grace_ends_at      TIMESTAMPTZ                   -- past_due: 7 days of grace
provider           TEXT                          -- NULL | 'stripe' | 'mercadopago'
provider_customer  TEXT
provider_ref       TEXT                          -- the gateway's subscription id
updated_at         TIMESTAMPTZ NOT NULL
```

Activation (`POST /v1/accounts/activate`) sets `trial_ends_at` for a
`trialing` row that has none.

### 5.2 Table `billing_events`

Append-only: account, old and new status, actor (`admin`, `system`,
`provider`), time, and a short reason. No card data ever reaches the server.
For audit and support. Erased with the account (`src/erase.rs`).

### 5.3 One entitlement function

`entitlement(row, now) -> Full | Frozen` is the only place that decides:

- `complimentary` → Full
- `active` with `current_period_end` in the future (or NULL) → Full
- `trialing` before `trial_ends_at` (or NULL, not yet activated) → Full
- `trialing` after `trial_ends_at` → Frozen
- `past_due` before `grace_ends_at` → Full; after → Frozen
- `frozen` → Frozen

Computing it rather than flipping the stored status means no scheduled job
has to fire on time. Every route calls this function, with the clock
injected for tests.

### 5.4 What the server enforces

Frozen answers `402 { "error": "account_frozen" }` on:

- `POST /v1/items` (create, edit, delete)
- `POST /v1/account/credentials`
- approving a pairing (phone-approved sign-in)
- `POST /v1/auth/login` from a device the account has never used

Still allowed, on purpose (data is never held hostage): read sync, login from
a known device, `GET /v1/vault/header`, device list and revoke, and
`POST /v1/account/delete`. Export is local and needs nothing from the server.

`GET /v1/sync` and `POST /v1/auth/login` add:

```json
"account": { "status": "trialing", "entitlement": "full",
             "trialEndsAt": "…", "periodEnd": null }
```

### 5.5 Admin CLI and the gateway seam

- `admin set-plan --email … --status complimentary|active|trialing|frozen [--until <date>]`,
  writing a `billing_events` row.
- `admin new-account` gains `--trial` or `--complimentary`; the default is
  `complimentary`, which keeps today's behaviour for invites.
- Migration: every existing `active` account gets a `complimentary` row.
- `src/billing/` exposes `apply_provider_event(account, event)`, the only
  writer besides the admin CLI. Stripe or Mercado Pago webhooks plug in there
  later without touching routes.

### 5.6 Scheduled emails

A daily task sends "your trial ends in 3 days" and "your trial has ended",
once per account each, marked in `billing_events`.

## 6. Apps

### 6.1 Onboarding (desktop and Android)

- The welcome screen gains **Create account**, which opens
  `https://havenkeys.net/signup` in the browser, and **I have a setup code**,
  the existing invite field with friendlier copy.
- Before the master password, the app shows the email and server the invite
  carries: "Creating an account for x@y on api.havenkeys.net".
- Then master password, Emergency Kit and confirmation, unchanged.

### 6.2 Status

- The client core (`havenkeys-client`) stores the `account` object from sync
  and login in the local replica (it is not secret) and exposes it to the UI
  and the native host.
- Trialing: a quiet "Trial: N days left" (desktop sidebar; Android
  Settings → Account).
- Frozen: a persistent "Your trial has ended. The vault is read-only." with
  **Subscribe**, which opens `https://havenkeys.net/account`. Until the gateway
  exists, the copy says to contact support.

### 6.3 Frozen behaviour

Decided in Rust from the stored entitlement; no UI decides on its own.

| Surface | Keeps working | Stops |
|---|---|---|
| Desktop | Open, search, reveal, copy, TOTP, export, delete account | Create, edit, delete, import, change master password |
| Extension | Popup: show and copy the **current site's** password and TOTP | Inline menu, Fill, automatic sign-in, sign-in-with, save prompts, generator insert, cards and identity fill |
| Android | Open, reveal, copy | System Autofill (the service answers with no datasets), saving |
| Passkeys | Signing in with a saved passkey | Creating a new passkey |

Signing in with a passkey stays because a passkey cannot be shown or copied:
blocking it would lock the user out of sites where they have no password.

The native host enforces it: when frozen, Rust refuses `fill_item` and the
menu's `find_matches` with `frozen`, refuses passkey creation, and still
answers the popup's reveal, which remains bound to the current origin.

### 6.4 Honest limitation (goes into `docs/security-model.md`)

The freeze of reads and autofill is enforced by open-source clients; a
modified build can remove it. The server guarantees only the refusal of
writes and of new devices.

## 7. Website (`apps/web`)

### 7.1 `/signup`

Three steps on one page:

1. Email, "I have read and accept the Terms and the Privacy Policy",
   **Create account**.
2. The 6-digit code, **Resend** after 60 s.
3. The setup code with **Copy**, download buttons per OS, and "We also
   emailed it. It is valid for 24 hours."

The invite lives only in page memory: never in `localStorage`, the URL or
analytics, and it is dropped when the page changes. The Vercel CSP allows
`connect-src https://api.havenkeys.net` and nothing else.

### 7.2 `/pricing`

The Personal plan, "14 days free, no card", and what happens afterwards.
The price reads "coming soon" until the gateway exists.

### 7.3 Legal

- Terms: trial, freeze, complimentary accounts, the right to export at any
  time.
- Privacy: the SMTP provider becomes an operator (LGPD); the email is used
  only for the code, the invite and account notices.
- `/delete-account` is unchanged.

### 7.4 Emails

pt-BR and en, plain text, no tracking pixels or tracked links: code, invite,
"you already have an account", "trial ends in 3 days", "trial ended".

## 8. Testing

Server:

- `start` answers identically for a new and an existing email; rate limits
  hold per IP and per email.
- A code expires, and dies after 5 wrong attempts; the stored hash is keyed.
- Racing `verify` calls on one code: one wins.
- `entitlement()` for every transition, with an injected clock.
- `402 account_frozen` on each write route and on a new device's login; read
  sync, known-device login, revoke and delete still pass when frozen.
- The migration turns existing active accounts `complimentary`.
- `HAVENKEYS_SIGNUP=off` answers `404` on both routes.
- Abandoned signups are swept after 7 days; activated accounts are not.

Client core and native host:

- Frozen refuses `fill_item`, the menu's `find_matches` and passkey creation.
- Frozen allows the popup reveal for the current origin and passkey sign-in.
- The wrong origin is still `DENIED` (attack 1 holds when frozen).

Extension: with a frozen account no inline menu appears and the popup offers
show and copy only.

Web: the three-step flow; the invite never reaches storage or the URL.

## 9. Out of scope

- Payment gateway, checkout page, price.
- `havenkeys://` deep link.
- Captcha.
- Family or team plans (they need sharing).
- Billing on self-hosted servers.
