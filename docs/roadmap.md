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

## 3. Server-authoritative vault: what's left

`docs/superpowers/specs/2026-09-20-server-authoritative-vault-design.md` is
the current design and supersedes the folder-sync model this document used
to describe here (`docs/server-sync.md` replaces `sync.md`; see its "Status"
section for exactly what exists in code today). Order of work, from the
design's §13:

1. **Core — done.** Schema 4, the single account-bound key scheme,
   `stage_create`/`stage_update`/`stage_delete`/`commit_write`, and the pull applier
   (`apply_remote_changes`) are in `havenkeys-core`. The folder-sync code,
   key schemes 1 and 2, the `tombstones` table, the `dirty` columns and the
   unauthenticated `deleted_at` hazard are deleted, not just deprecated. The
   vault is unusable for writes until a server and a sync client exist,
   which is expected at this step.
2. **`havenkeys-server` — code done, not deployed.** The crate exists with
   the schema, activation, sessions, the vault header, pull, optimistic
   per-item writes, devices and the admin CLI, tested against a real
   Postgres (`scripts/test-server.sh`): account isolation on every
   authenticated route, uniform answers from `auth/params`, rate limiting,
   size and shape limits, and a `no_logging` guard. `cargo audit` and
   `cargo deny check` are clean.

   **Still owed, and blocking before any real vault is stored:** the deploy
   itself (`docs/deployment.md` has the image, the Railway service
   definition, the environment and the first-account steps), confirming the
   health check and TLS on the deployed host, and the **restore drill** in
   `docs/deployment.md` §5. The local replica is not a backup, so an
   untested backup means the vault has none.
3. **`havenkeys-sync-client` — next.** The HTTP client and the
   hostile-server test suite: a header whose attestation does not verify, a
   header below `max_header_rev`, a blob that fails to open, an oversized
   blob or body, a pull that deletes everything.
4. **Desktop — partially started.** This step (Task 7) added the
   online/offline distinction, the read-only gate on every mutating
   command, and the offline banner (`docs/server-sync.md` §4). Still
   needed: activation from an invite, second-device sign-in, the Emergency
   Kit v2 screen (account, email, server URL, not just the vault ID), and
   Account settings (email, server, device list, revoke device, sign out).
   There is no folder-picker UI left to remove — it went with the
   folder-sync code.
5. **Docs — in progress.** This step rewrote `crypto.md`, `server-sync.md`,
   `architecture.md` and this file. Still owed, once the server and sign-in
   actually ship (not before — a doc should not describe behaviour the code
   doesn't have): `CLAUDE.md` §1 ("works completely without an internet
   connection" → "reads work offline; changes require the server", "no
   backend required" removed), `README.md`, `threat-model.md` and
   `security-model.md` (the design's §10). Then a security review pass over
   the whole change, written to `security-review.md`.

Carried forward, true today:

* **A hostile or compromised server can destroy data, and that is now
  inherent, not a flaw to fix.** The server is the authority; a deletion it
  serves is indistinguishable from a real one by construction (design §9).
  **Tested server backups are therefore a prerequisite of shipping**, not a
  follow-up — the local replica is a cache of the server, not an
  independent copy the way each device's SQLite was under the folder model.
* **Re-pointing a vault** at a different account or server has no path;
  `Store::set_account` refuses it. Decide this in the sync-client plan.
* The vault key is still never rotated (#8), and the Secret Key is still
  stored in plain text in `device.json` (`docs/server-sync.md` §7).

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
* **Relationship to the desktop vault:** sync is now server-authoritative
  (`docs/server-sync.md`), not a shared folder, so a standalone extension
  would need its own `havenkeys-sync-client` session against the same
  server account rather than file-system access — either a separate vault,
  or its own login sharing the account's items through the server.

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
