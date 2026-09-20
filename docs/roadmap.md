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
the design. Steps 1–5 of its §13 have landed; what remains is not code.

1. **Core — done.** Schema 4, the single account-bound key scheme, the
   staged write path (`stage_create`/`stage_update`/`stage_delete`/
   `stage_save_login`/`stage_import` → `commit_write`), the pull applier and
   `reset_sync_cursor`.
2. **`havenkeys-server` — code done, not deployed.** Activation, sessions,
   the header, paged pull, optimistic writes, devices, the admin CLI, and
   the suites in `scripts/test-server.sh` against a real Postgres.
3. **`havenkeys-sync-client` — done.** The transport, the wire types, the
   hostile-server suite, and a round trip against the real server with real
   core crypto.
4. **Desktop — done.** Activation from an invite, second-device sign-in, the
   Emergency Kit v2, Account settings (devices, revoke, sign out, sync,
   re-download), the offline banner and the read-only gate. The browser
   extension's save-login writes through the same path.
5. **Docs — done.** `CLAUDE.md` §1, `README.md`, `threat-model.md`,
   `security-model.md`, `crypto.md`, `architecture.md`, `server-sync.md`,
   `deployment.md` and this file, plus the review in `security-review.md`.

**Blocking before anyone stores a real vault:**

* **Deploy the server** and confirm the health check and TLS on the deployed
  host (`docs/deployment.md` §3).
* **Run the restore drill** (`docs/deployment.md` §5). The local replica
  follows the server, deletions included, so an untested backup means the
  vault has none. This is the mitigation for `security-review.md` S5, and
  until it has run there is none.
* **Use it on two real computers.** Nothing has yet crossed a real network:
  the round trip runs both devices in one process.

Carried forward, true today:

* **A hostile or compromised server can destroy data, and that is inherent,
  not a flaw to fix** (design §9, `threat-model.md` T1c).
* **Re-pointing a vault** at a different account or server has no path;
  `Store::set_account` refuses it.
* The vault key is still never rotated (#8), and the Secret Key is still
  stored in plain text in `device.json` (`docs/server-sync.md` §7).
* A pulled item that does not open is skipped and the cursor still advances;
  the way back is Settings → Account → Re-download everything, which nothing
  offers automatically (`security-review.md` S9, `server-sync.md` §7).

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
