# Self-Service Signup and Plans — Plan Index

**Spec:** `docs/superpowers/specs/2026-10-07-self-signup-and-plans-design.md`

The spec touches four codebases that ship separately: the server, the
website, the client core with its three shells, and their documentation. Each
stage below leaves the product releasable on its own. Stages 1 and 2 are
planned now; stage 3 is planned once stage 1 is merged, because it builds on
the `account` object and the `402` the server then answers with, and on
names in the client core that a survey can only confirm against the code of
the day.

| Stage | Plan | Status | Depends on |
|---|---|---|---|
| 1. Server: plans, entitlement, signup, emails | `2026-10-09-self-signup-stage1-server.md` | Planned | — |
| 2. Website: `/signup`, `/pricing`, legal | `2026-10-09-self-signup-stage2-website.md` | Planned | Stage 1's routes on `api.havenkeys.net` to work live; can be built and tested before |
| 3. Apps: account status, frozen behaviour, onboarding | `2026-10-09-self-signup-stage3-apps.md` | Done (commits `c7d2916..ae1228e`) | Stage 1 |

Stage 1 can ship alone: with `HAVENKEYS_SIGNUP=off` (the default) nothing
changes for users; every existing account becomes `complimentary` and the
apps ignore the new `account` field (sync-client responses tolerate unknown
fields). Opening signup on the hosted server waits for stage 2 (the page)
and stage 3 (the apps show the trial and honour a freeze).

## Order of deployment

1. Merge stage 1; deploy the server with `HAVENKEYS_SIGNUP=off` and
   `HAVENKEYS_CORS_ORIGIN=https://havenkeys.net` (harmless with signup off; it
   lets the page show the sign-up-closed message instead of a network error).
   Run `admin list-accounts` and confirm every account shows `complimentary`.
2. Merge stage 2; deploy the site. `/signup` answers "Sign-up opens soon" (the
   closed message) until step 4.
3. Merge stage 3; release desktop, extension and Android.
4. Set `HAVENKEYS_SIGNUP=open`, `HAVENKEYS_PUBLIC_URL`, `SMTP_URL`
   and `SMTP_FROM` on the hosted server. Create one account through the page
   end to end.

## What the stage 3 plan must cover (spec §6, §8 client bullets, §3)

Read first: `crates/havenkeys-sync-client/src/wire.rs` (`LoginDto`,
`PullDto`), `crates/havenkeys-sync-client/src/client.rs` (`error_for`,
`login`, `pull`), `crates/havenkeys-client/src/sync.rs` (`connect`,
`sync_now`, `failed`, `push`), `crates/havenkeys-core/src/store.rs`
(`account` table, `AccountRecord`, `SCHEMA_VERSION`),
`crates/havenkeys-bridge/src/dispatch.rs` and `server.rs`,
`crates/havenkeys-protocol/src/message.rs` (`ResultBody::Status`,
`ErrorCode`), `packages/protocol/src/index.ts` (`status` parser,
`ERROR_CODES`), and the survey notes below.

### Client core

- `LoginDto` and `PullDto` gain an optional `account: Option<AccountDto>`
  (`status`, `entitlement`, `trial_ends_at`, `period_end`); `error_for`
  maps `402` to a new `SyncError::AccountFrozen`, and `ClientError` gets the
  code `account_frozen` (desktop `errors.ts`, Android `ErrorText.kt`).
- The core store's `account` table (plaintext, non-secret metadata) gains
  `plan_status`, `entitlement`, `trial_ends_at`, `period_end`,
  `plan_seen_at`: `SCHEMA_VERSION` 6 → 7 with a migration in `Store::init`
  (`MIGRATE_6_TO_7`). The vault is resettable but other users' devices are
  not: write the migration.
- `HavenClient::account_status()` → `AccountStatus` gains the same fields;
  `havenkeys-mobile`'s `Status` record too.
- A core-level `require_full()` next to `require_online()`, called by
  `push`, `push_batches`, `change_master_password`, `import_file`,
  `restore_backup` and the passkey-create path, so a frozen account fails
  locally with `account_frozen` before any request (the server refuses too;
  the local check is for the message and for offline).
- Frozen is decided from the stored entitlement in Rust; the UI reads it.

### Bridge (extension requests)

- `ResultBody::Status` gains `entitlement: "full" | "frozen"` (Rust and
  `packages/protocol`; the parser's `hasExactKeys` list). It is sent only
  when frozen, so an older extension keeps working for Full accounts. The
  native host is relay-only; it ships as the desktop's sidecar
  (`externalBin` in `apps/desktop/src-tauri/tauri.bundle.conf.json`), not in
  `pnpm package:extension`, and a dev machine set up with
  `scripts/install-native-host.sh` must rebuild it.
- `ErrorCode::Frozen` (snake_case `frozen`), with its fixed English message
  mirrored in the extension's `bridge-parity.test.ts`.
- When frozen, `dispatch` refuses with `Frozen`: `find_matches` (so no menu
  opens), `fill_item`, `start_sso`, `save_login`, `save_card`, `save_sso`,
  `fill_identity`, `fill_card`, `generate_password` insertion path
  (`generate_password` itself may still answer: the generator does not
  touch the vault; decide and test), `check_passkey_create`,
  `passkey_create`. Still answered: `status`, `lock`, `show_unlock`,
  `get_totp` (origin-bound), `find_passkeys`, `passkey_get`, `open_item`.
- Spec §6.3 wants the popup to "show and copy the current site's password".
  There is no reveal request today; the popup only fills. Decision needed
  before writing the stage 3 plan: (a) add a `reveal_item { item_id, url,
  top_url }` request answered only with the password, origin-bound like
  `fill_item`, with the popup rendering a masked field and Copy, or (b)
  keep the popup to TOTP plus "Open in desktop" when frozen, since the
  desktop reveals and copies. (b) is smaller and adds no new secret path
  through the extension; (a) matches the spec's table literally.
- Attack 1 must hold when frozen: wrong origin stays `denied`, not `frozen`
  (origin is checked first).
- Tests in `crates/havenkeys-bridge/tests/bridge.rs` next to `a3_locked_vault_refuses`.

### Extension

- `popup-handler.ts` `stateForError`/`state()` map `frozen` to a new
  `PopupState { kind: "frozen"; site; matches }` that lists the current
  site's logins with Code only (or with Show/Copy if decision (a)), and a
  "Your trial has ended" line with a button to `https://havenkeys.net/pricing`.
- `inline-handler.ts` `openMenu`: a `frozen` error from `find_matches`
  returns `{ ok: false }` (no menu), like any other error; add the test.
- Save prompts, generator insert, automatic sign-in, sign-in-with, card and
  identity fill all go through requests the bridge now refuses; the
  extension must not show a prompt it cannot complete: `check_login` and
  `check_passkey_create` answer `frozen` and the handlers drop the prompt.
- `bridge-parity.test.ts` and the pt-BR message for `frozen`.

### Desktop

- `WelcomeScreen.tsx`: **Create account** button (opens
  `https://havenkeys.net/signup` through a new fixed-URL Tauri command,
  registered in `build.rs`, `capabilities/main.json` and
  `generate_handler!`), and the invite tab renamed **I have a setup code**
  with the textarea placeholder and copy updated.
- Before the master password: a new `preview_invite(invite) -> { email,
  server }` command (decode only, no network) so the panel shows "Creating
  an account for x@y on api.havenkeys.net".
- `VaultScreen.tsx` sidebar foot: "Trial: N days left" when `trialing`; a
  persistent banner when frozen with **Subscribe** (opens
  `https://havenkeys.net/pricing`) and the copy "Your trial has ended. The
  vault is read-only."
- `readOnly` becomes `!device?.online || account?.entitlement === "frozen"`
  so New, editors, import and change-password are disabled with the frozen
  message; export and delete account stay enabled.
- `errors.ts` and `en.ts`/`pt-BR.ts`: `account_frozen`.

### Android

- `OnboardingScreen.kt` `ChooseStep`: **Create account** (opens the signup
  URL in the browser) and the invite choice renamed; `InviteStep` shows the
  decoded email and server before the password.
- `SettingsScreen.kt` `AccountGroup`: the plan row ("Trial: N days left" /
  "Trial ended. Read-only.") with Subscribe.
- `HavenAutofillService.respond` returns `null` when the Rust status says
  frozen (no datasets); `onSaveRequest` does nothing. Passkey creation
  (`PasskeyCreateViewModel`) shows the frozen error.
- Editors, trash, save flows surface `account_frozen` through `ErrorText.kt`.
- Strings in `values` and `values-pt-rBR`.

### Docs

- `docs/security-model.md` §6.4 honest limitation (clients enforce the
  freeze of reads and autofill; a modified build can remove it) if stage 1
  did not already add it.
- `docs/native-messaging.md`: the `entitlement` field and the `frozen` code.
- `docs/security-review.md`: an entry for the new reveal request if (a).
- `docs/ideas.md` item 13 closed.

## Stage 3 outcome

Done. Decisions taken while building, which settle the open questions above:

- Subscribe opens `https://havenkeys.net/pricing`; Create account opens
  `/signup`. Both are fixed URLs.
- No `reveal_item` request: the frozen popup offers TOTP and Open in desktop.
- `find_matches` is answered while frozen (metadata, origin bound) and
  carries `entitlement`; the inline menu honours it. `generate_password` is
  still answered. The refused list is `refused_when_frozen` in
  `crates/havenkeys-bridge/src/dispatch.rs`; `fill_item` checks the origin
  first, then the plan.
- The store keeps the plan in `PlanRecord`, separate from `AccountRecord`; a
  missing plan reads as `full`. `plan_changed` is a default no-op method on
  the client events, so shells that do not show the plan need no change.
- Release order: ship the extension (its parser tolerates a missing
  `entitlement`) before or with the desktop. An old extension against a new
  desktop must update.
- Docs updated: `native-messaging.md`, `security-model.md` §25,
  `security-review.md` (FZ1 to FZ4), `android.md`.
