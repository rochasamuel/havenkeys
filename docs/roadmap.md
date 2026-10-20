# Roadmap

What is left after Phases 1–6, roughly in order. Security findings referenced
here are tracked in `security-review.md`.

> This software has not undergone an independent security audit.

> **Reaching 1Password-level feature coverage is mapped in
> `docs/1password-parity.md`** — a topic-by-topic comparison and a spec index
> linking each gap to its draft design spec in `docs/superpowers/specs/`
> (dated 2026-10-13). Autofill specifically is in `docs/autofill-roadmap.md`.

> **Autofill, extension, transport and passkeys are tracked in
> `docs/autofill-roadmap.md`** — the single home for all of it. Items that
> used to live in this file (the Phase 5 browser/Windows verification, the
> passkey checklist, pipe squatting, the F2 desktop-confirmation residual,
> standalone extension mode, and the OS credential-provider / passkey
> import-export work) now live there. This file keeps only the non-autofill
> work.

## 1. Verify on Windows (non-autofill)

* Runtime check of the desktop CSP and capabilities (#14).
* Windows: vault locking on Win+L (H1).
* (Browser/extension and named-pipe verification — Phase 5 checklist, pipe
  DACL P1, pipe squatting BR-1, the passkey checklist: see
  `docs/autofill-roadmap.md` §E1 and §D1.)

## 2. Finish Phase 6 (hardening)

* **macOS screen lock**, if macOS becomes a target (lock, not fill).
* A full, whole-system security review before calling the MVP done
  (CLAUDE.md §55, §57).
* **Remove the legacy auth-key login after the OPAQUE cutoff** (spec
  `2026-10-13-pake-login-design.md` §6.5). Once `HAVENKEYS_LEGACY_AUTH_UNTIL`
  has passed on the hosted server and `admin list-accounts` shows no
  `legacy` account worth keeping, delete: `/v1/auth/login`, the `authKey` /
  `currentAuthKey` / `newAuthKey` fields, `/v1/account/pake/enroll`,
  `admin allow-legacy-sign-in`; `hash_auth_key`, `verify_auth_key`,
  `dummy_verifier` and the server's Argon2 dependency; the
  `accounts.auth_verifier` and `legacy_grace_until` columns (migration);
  `login_legacy`, `activate_legacy` and the fallback in
  `havenkeys-client/src/pake.rs`. Keep `pake_confirmed`. Reminder set for
  2026-11-17.
* (Desktop confirmation for browser-initiated password changes, F2 residual:
  see `docs/autofill-roadmap.md` §E2.)

## 3. Server-authoritative vault: what's left

`docs/superpowers/specs/2026-09-20-server-authoritative-vault-design.md` is
the design. Steps 1–5 of its §13 have landed; what remains is not code.

1. **Core — done.** Schema 4, the single account-bound key scheme, the
   staged write path (`stage_create`/`stage_update`/`stage_delete`/
   `stage_save_login`/`stage_import` → `commit_write`), the pull applier and
   `reset_sync_cursor`.
2. **`havenkeys-server` — done, and deployed.** Activation, sessions, the
   header, paged pull, optimistic writes, devices, the admin CLI, the
   credential-change and item-fetch routes, and the suites in
   `scripts/test-server.sh` against a real Postgres.
3. **`havenkeys-sync-client` — done.** The transport, the wire types, the
   hostile-server suite, and a round trip against the real server with real
   core crypto.
4. **Desktop — done.** Activation from an invite, second-device sign-in, the
   Recovery Sheet v2, Account settings (devices, revoke, sign out, sync,
   re-download), the offline banner and the read-only gate. The browser
   extension's save-login writes through the same path.
5. **Docs — done.** `CLAUDE.md` §1, `README.md`, `threat-model.md`,
   `security-model.md`, `crypto.md`, `architecture.md`, `server-sync.md`,
   `deployment.md` and this file, plus the review in `security-review.md`.

**Blocking before anyone stores a real vault:**

* **Run the restore drill** (`docs/deployment.md` §5). The local replica
  follows the server, deletions included, so an untested backup means the
  vault has none. This is the mitigation for `security-review.md` S5, and
  until it has run there is none.
* **Use it on two real computers.** Nothing has yet crossed a real network:
  the round trip runs both devices in one process.

Carried forward, true today:

* **A hostile or compromised server can destroy data, and that is inherent,
  not a flaw to fix** (design §9, `threat-model.md` T1c).
* The vault key is still never rotated (#8).
* **Moving items between accounts has no path.** "Remove this device" plus
  Activate/Sign in fresh is how a device changes which server or account it
  points at; there is no path that migrates or merges items between accounts.

## 4. Missing MVP features

* ~~**Export** with the plaintext warning and explicit confirmation
  (CLAUDE.md §38).~~ Done (`2026-10-05-export-design.md`).

## 5. Standalone extension mode (proposal, needs a decision)

Moved to `docs/autofill-roadmap.md` §E4 (it is an extension/autofill change).
In short: run `havenkeys-core` as WebAssembly inside the extension so it works
without the desktop app — a deliberate security-model change (the extension
would then hold the vault), opt-in, recorded in the threat model. Full write-up
(crypto, storage, session, what gets weaker, suggested approach) is in the
autofill roadmap.

## Later (out of MVP scope)

A hosted sync service, sharing. See CLAUDE.md for the current scope.

Passkeys are in scope since 2026-09-23 (CLAUDE.md, amendment), as a provider
for websites. Later passkey work — import/export in CXF, OS credential-provider
integration — is tracked in
`docs/autofill-roadmap.md` §D (passkeys as a fill surface).
