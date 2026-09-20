# Roadmap

What is left after Phases 1–6, roughly in order. Security findings referenced
here are tracked in `security-review.md`.

> This software has not undergone an independent security audit.

## 1. Verify in real browsers and on Windows

Nothing from Phase 5 has run in a real browser yet, and the Windows
screen-lock code has not run on Windows.

* Work through the Phase 5 checklist in `security-review.md` in Chrome and
  Firefox: popup Fill, the options toggle, the menu on sites with strict CSPs,
  save and update prompts, and the overlay (clickjacking) test on Chromium.
* Runtime check of the desktop CSP and capabilities (#14).
* Windows: the named-pipe DACL (P1), and the vault locking on Win+L (H1).

## 2. Finish Phase 6 (hardening)

* **Pipe owner check (P10):** before talking to the named pipe, the native
  host verifies that the pipe's server process runs as the same user
  (`GetNamedPipeServerProcessId`, then the token owner).
* **Desktop confirmation for browser-initiated password changes (F2
  residual):** the desktop app asks before accepting `save_login` updates,
  so a compromised extension cannot cycle a password out of the history.
* **macOS screen lock**, if macOS becomes a target.
* A full, whole-system security review before calling the MVP done
  (CLAUDE.md §55, §57).

## 3. Secret Key and sync (built, needs a real-world run)

Done: Secret Key and Emergency Kit, and sync through a shared cloud folder
with a new device joining with password + Secret Key (`sync.md`). To check:
create a vault (or add a Secret Key), save the kit, choose a OneDrive folder,
join from a second computer, then edit and delete on both sides.

Next for sync:

* **Mobile app:** reuse `havenkeys-core` through UniFFI, scan the Emergency
  Kit QR, and keep the Secret Key in the platform keystore (`sync.md` §7).
* Optional later: rotate the vault key (#8), use hybrid logical clocks
  instead of wall clocks for conflicts (K3), and add per-device approval on
  top of the Secret Key.

### Server accounts and sync (key scheme 3, in progress) — carry-forwards

`havenkeys-core` has key scheme 3 and the delta sync merge/cursor logic
(`docs/superpowers/specs/2026-09-19-server-accounts-sync-design.md`), but no
server and no sync client yet (order of work, spec §13). These cross plan
boundaries and would otherwise be lost between here and there:

* **Desktop "needs Secret Key" plumbing does not admit scheme 3 yet.**
  `apps/desktop/src-tauri/src/account.rs` computes `needs_secret_key` as
  `key_scheme == Some(KeyScheme::PasswordAndSecretKey)`, and
  `apps/desktop/src/lib/types.ts` declares a **closed** union
  `KeyScheme = "password_only" | "password_and_secret_key"` (consumed in
  `App.tsx` and `views/SyncSection.tsx`) that has no variant for
  `"account_bound"`. Once anything creates a scheme-3 vault, `device_status`
  will serialize a `key_scheme` value the TypeScript type does not admit.
  Unreachable today only because nothing in production code creates a
  scheme-3 vault yet.
* **`VaultService::change_master_password` has no scheme-3 route.** It (and
  the desktop `change_master_password` command, `commands.rs`) goes through
  `RekeyTicket::derive`/`derive_with_secret_key`, which derives the KEK with
  `account: None` and is refused for `KeyScheme::AccountBound`. The method
  that does work, `RekeyTicket::derive_for_account`, has no `VaultService` or
  desktop-command caller. A scheme-3 user pressing "change master password"
  hits a dead end on a security-critical operation. Needs a
  `change_master_password_for_account` (or equivalent) route wired to the
  desktop UI before scheme 3 ships there.
* **`Store::set_account`'s `ON CONFLICT` clause is a latent trap for a
  changed account.** It deliberately preserves `server_cursor` and
  `max_header_rev` while updating `account_id`, `email` and `server_url` —
  correct for re-signing into the *same* account, but if a future flow calls
  it with a *different* `account_id` or `server_url` (e.g. re-pointing a
  vault at a different account or server), the stale cursor makes the first
  pull silently skip everything before it, and the stale `max_header_rev`
  floor makes every future header from the new account fail the rollback
  check in `adopt_account_header`/`prepare_sign_in`. Latent today because
  `create_vault` refuses a store that already holds a header, so `set_account`
  is never called against a store that already has one. Decide this in the
  desktop/sync-client plan (reset the row vs. a distinct "re-point" path)
  rather than discover it as a bug there.
* **Nothing calls `Store::set_account` in production yet.** Both activation
  and sign-in must call it (`VaultService::store_account`), or the durable
  `max_header_rev` rollback floor described in `crypto.md` and exercised in
  `tests/account.rs` is never populated outside tests, and header-rollback
  protection reduces to the in-memory `local.revision` alone.

## 4. Missing MVP features

* **Export** with the plaintext warning and explicit confirmation
  (CLAUDE.md §38).

## 5. Standalone extension mode (proposal, needs a decision)

**Idea:** the browser extension works without the desktop app. The user
creates or unlocks a vault and enters the master password in the extension
itself, and manages logins there.

**This changes the security model.** Today the extension never sees the
master password, the vault key, or the database. It can only ask the desktop
for one login at a time, for the page it is on, and Rust enforces that.
CLAUDE.md requires this split explicitly: the master password is never sent
to the extension (§9, rule 11), and the extension never touches the database,
the vault key or the master password (§30, rules 8 and 12). A standalone mode
reverses those rules for users who opt into it, so it has to be a deliberate
decision, recorded in the threat model, and not an incremental feature.

### What it would take

* **Crypto:** compile the existing Rust core (`havenkeys-core`) to
  WebAssembly and run it in the extension's background worker. This keeps
  one audited implementation, with the same Argon2id, AES-256-GCM, vault
  format and origin binding. Nothing is reimplemented in TypeScript.
* **Storage:** the encrypted vault (same blob format) in the extension's
  IndexedDB. Only ciphertext and the unlock header are stored. Plaintext is
  never stored, and keys are never stored.
* **Unlock UI:** the master password is typed only into an extension page
  (the popup or a dedicated tab), never into a content script or anything a
  web page can reach.
* **Unchanged:** content scripts, the in-page menu and the save flow keep
  working exactly as now. They talk to the background worker, which would
  answer from the in-extension vault instead of the native host, through the
  same origin-bound API.
* **Session:** keys live in WASM memory in the background worker. MV3 stops
  idle service workers, which drops the session. That means either
  re-unlocking often, or keeping the unlocked key in `storage.session`. That
  store is memory-only but readable by any extension context, so the choice
  needs to be made explicitly.
* **Argon2id in WASM** is slower than native. Parameters must be benchmarked
  in browsers, not assumed (CLAUDE.md §5).
* **Relationship to the desktop vault:** the folder sync (`sync.md`) needs
  direct access to a folder on disk, which browser extensions do not have.
  A standalone extension would be either a separate vault, or it would need
  another way to reach the same encrypted files: a user-picked directory
  through the File System Access API (Chromium only), or a hosted relay.

### What gets weaker

* **Compromise of the extension exposes the whole vault** while it is
  unlocked, instead of one login per page visit within rate limits. That
  includes a malicious extension update, a bug in extension code, or a
  browser exploit in the extension process.
* **Memory hygiene is weaker.** WASM linear memory can be zeroized, but the
  password typed into an extension page passes through JS strings that
  cannot be wiped.
* **No OS-level lock signals.** The extension cannot see screen lock or
  suspend the way the desktop can. It would rely on idle timers and
  `idle.onStateChanged`, which adds the `idle` permission.
* **More permissions:** at least `storage` (IndexedDB) and `unlimitedStorage`
  for large vaults.

### Suggested approach if pursued

1. Make it opt-in and clearly labelled as a different security trade-off,
   with the desktop app remaining the recommended setup.
2. Build the WASM core first and run the existing core test suites against
   the WASM build.
3. Put a "vault backend" interface in the background worker with two
   implementations: native host (today) and in-extension WASM. Content
   scripts and the in-page UI stay unchanged.
4. Update `threat-model.md`, `security-model.md` and `crypto.md` (keys in the
   browser, session lifetime, storage) before shipping.

## Later (out of MVP scope)

A hosted sync service, passkeys, sharing. See CLAUDE.md for the
current scope.
