# PAKE login — design

Date: 2026-10-13, revised 2026-10-17. Status: **approved 2026-10-17**
(OPAQUE; 30-day dual-login window; post-quantum TLS ships separately).

Replaces sending the auth key to the server with OPAQUE (RFC 9807), a
Password-Authenticated Key Exchange, so the server never receives a
credential that it, or anyone who sees a request, could replay. See
`docs/1password-parity.md` item #9.

## 1. Goal and honest scope

Today a device proves who it is by **sending its 32-byte auth key** inside
TLS; the server keeps an Argon2id hash of it. The auth key decrypts nothing —
the vault needs the master password and the Secret Key, which never leave the
device — but it is a **bearer secret in flight**. Whoever sees it once (a
proxy that logs bodies, a mis-issued certificate, the server's memory at
receipt) can sign in as the user until the master password changes, and with
that session download the ciphertext, change or delete items, register
devices, or delete the account.

Goal: the device proves knowledge of the auth key **without sending it**;
nothing seen on the wire or in the server's database can be replayed to sign
in.

What this does **not** do, and the docs must not claim:

* It does not protect vault contents more than today; they never depended on
  the auth key.
* It does not protect against a server that is already hostile: such a
  server can destroy data anyway (`threat-model.md` T1c).
* It does not replace TLS.

Non-goals: the KEK / vault-key hierarchy (`crypto/keys.rs`), the blob format,
session tokens and their TTL, devices, pairing, and the extension/native-host
boundary are unchanged.

## 2. What exists today

* `havenkeys-core/src/crypto/keys.rs:121` `derive_auth_key_from_master`:
  HKDF-SHA256 over `master_key ‖ Secret Key`, salted by the account, label
  `havenkeys/v3/auth` → `AuthKey` (32 bytes).
* `havenkeys-server/src/auth/mod.rs`: `hash_auth_key` / `verify_auth_key`
  (Argon2id 19 MiB × 2, PHC string in `accounts.auth_verifier`), the
  `dummy_verifier`, the verify semaphore (SV-4), `issue_token`.
* **Every route that receives an auth key** (all must change):

  | Route | Field | Purpose |
  |---|---|---|
  | `POST /v1/accounts/activate` (`routes/accounts.rs:144`) | `authKey` | enrolment (invite and self-signup both end here) |
  | `POST /v1/auth/login` (`routes/auth.rs:122`) | `authKey` | sign-in |
  | `POST /v1/account/credentials` (`routes/account.rs:51`) | `currentAuthKey`, `newAuthKey` | prove the old key, enrol the new one |
  | `POST /v1/account/delete` (`routes/account.rs:150`) | `currentAuthKey` | re-authenticate before erasing |

* `POST /v1/auth/params` returns the account id (a decoy for unknown emails,
  from `SERVER_SECRET`) and the KDF parameters.
* Not affected: `/v1/pairings/*` issues the new desktop's session without an
  auth key (`routes/pairings.rs:313`); the Android unlock bundle
  (`core/src/unlock_bundle.rs`) stores the auth key on the device and keeps
  working, because the auth key becomes the OPAQUE input.

The favourable fact: the OPAQUE "password" is **the existing `AuthKey`**, 256
bits already stretched with Argon2id on the client. There is no human secret
to grind, so OPAQUE's own key-stretching function can be the identity.

## 3. Decisions

| Question | Decision | Why |
|---|---|---|
| Which PAKE | **OPAQUE, crate `opaque-ke` 4.x** | RFC 9807; audited by NCC Group (2021, for WhatsApp backups); ~270k recent downloads. RustCrypto `srp` says it has *never* been independently audited ("use at your own risk"), which CLAUDE.md §48 rules out. |
| Cipher suite | ristretto255 OPRF, ristretto255 3DH key exchange, SHA-512, KSF = `Identity` | Constant-time curve arithmetic (curve25519-dalek). `Identity` KSF because the input is already Argon2id-stretched and 256 bits; a second stretch on the client only slows unlock. |
| Password input | `AuthKey` bytes, verbatim | No new KDF. |
| Credential identifier | the account id (UUID), not the email | Stable, and `auth/params` already hands the client one (a decoy for unknown emails). |
| Server long-term secret | a `pake_server_setup` row in Postgres, generated once at first start | Goes into the same backups as the records it opens. **Losing it locks every enrolled account out** — the restore drill must cover it. Not derived from `SERVER_SECRET`, whose docs promise it is safe to change. |
| Stored per account | `accounts.opaque_record` (the registration upload) | `auth_verifier` stays during the window (§6). |
| Session | today's random 32-byte token, issued after the proof | OPAQUE's session key and export key are **not used** for anything. |
| Unknown email | `ServerLogin::start` with no record → `opaque-ke`'s fake credential response, deterministic per identifier | Same answer shape and timing as a real account. |

## 4. Protocol

All OPAQUE code sits in one small new crate, `havenkeys-opaque` (the suite,
byte-level wrappers, size checks), used by the server directly and by the
client only through `havenkeys-core::pake`, so the auth key's bytes never
leave the core crate. The key-exchange context is `havenkeys/opaque/v1`.

### 4.1 Routes

| Route | Auth | Body → answer |
|---|---|---|
| `POST /v1/auth/pake/start` | none | `{email, credentialRequest, deviceId}` → `{loginId, credentialResponse}` |
| `POST /v1/auth/pake/finish` | none | `{loginId, credentialFinalization, deviceId, deviceName}` → today's login answer |
| `POST /v1/accounts/activate/register` | invite | `{email, invite, registrationRequest}` → `{registrationResponse}` |
| `POST /v1/account/pake/register` | session | `{registrationRequest}` → `{registrationResponse}` |
| `POST /v1/account/pake/reauth` | session | `{credentialRequest}` → `{reauthId, credentialResponse}` |
| `POST /v1/account/pake/enroll` | session | `{currentAuthKey, registrationUpload}` → 204 |

Changed bodies (exactly one of each pair):

* `accounts/activate`: `authKey` **or** `registrationUpload`.
* `account/credentials`: proof `currentAuthKey` **or** `{reauthId,
  credentialFinalization}`; new key `newAuthKey` **or** `registrationUpload`.
* `account/delete`: proof `currentAuthKey` **or** `{reauthId,
  credentialFinalization}`.

`ServerRegistration::start` is stateless (derived from the server setup and
the credential identifier), so registration needs no pending state.

### 4.2 Sign-in

1. `auth/params` → unchanged (account id, KDF).
2. `auth/pake/start`: the server loads the record (none for an unknown
   email or an unenrolled account), runs `ServerLogin::start`, stores the
   serialized state in `pake_pending` under the SHA-256 of a random 32-byte
   `loginId` (purpose `login`, account id or NULL, `expires_at` now + 60 s),
   charges the rate limit (`AttemptKeys`, account id or decoy id, and the
   address) and answers. If no active account has that email and `deviceId`
   belongs to a deleted account, it answers **410 `account_deleted`** right
   after charging the rate limit, exactly as the legacy login does for an
   unknown email: a device that was locked when its account was deleted
   learns it here, because its OPAQUE sign-in would otherwise fail on the
   device and send nothing more (amended 2026-10-17).
3. The client finishes; a wrong master password or Secret Key, or a fake
   response, fails **on the device** and nothing more is sent.
4. `auth/pake/finish`: the pending row is **deleted on read** whatever
   happens next; expired or unknown → 401; `ServerLogin::finish` fails →
   401 (or 410 for a deleted device, as today); success → the rest of
   today's `login` unchanged (rate limit cleared, frozen-account refusal for
   a new device, `register_device`, old session dropped, `issue_token`, plan).

### 4.3 Re-authentication

`account/pake/reauth` is the same exchange inside a session (purpose
`reauth`, bound to that session's account id). `credentials` and `delete`
consume the row the same way (single use, 60 s, account must match).

### 4.4 Rules per account state

| | Legacy proof (`currentAuthKey`, legacy login) | OPAQUE proof |
|---|---|---|
| No `opaque_record` | allowed until the cutoff | 401 (fake record) |
| Has `opaque_record`, has `auth_verifier` | **login only** (old devices); credentials and delete answer 426 | allowed |
| Has `opaque_record`, `auth_verifier` NULL | 401 | allowed |

* `newAuthKey` is accepted only with a legacy proof on an unenrolled
  account. `registrationUpload` in `credentials` sets `opaque_record` and
  **clears `auth_verifier`** (an old device holding the old key could not
  sign in anyway; it must update).
* Activation with `registrationUpload` creates the account with no
  `auth_verifier`.
* `enroll` needs a valid legacy `currentAuthKey` (a stolen session alone
  must not choose the account's OPAQUE key) and only works while
  `opaque_record` is NULL; it keeps `auth_verifier` so old devices keep
  working until the cutoff.
* The optimistic lock in `credentials` / `delete` compares both
  `auth_verifier` and `opaque_record`.

## 5. Size and validation

Every new body stays under `MAX_ANONYMOUS_AUTH_BODY_BYTES` (4 KiB), has
`deny_unknown_fields`, and each OPAQUE field is base64 of at most 512 bytes
before `opaque-ke` deserializes it (which checks exact lengths). A malformed
OPAQUE message is a 400 with no detail. The hostile-server suite gains cases
for a malformed, oversized or reflected `credentialResponse`.

## 6. Migration (there are real accounts besides the owner's)

The server cannot tell an app's version, so:

1. **Server N.** Ships everything above; the legacy paths keep working.
2. **Clients N.** Desktop and Android sign in with OPAQUE first. If that
   fails with 401 (no record, or a wrong password) or 404 (a server older
   than N), and **this device has never signed in to this account with
   OPAQUE**, they try the legacy login; if that succeeds they call `enroll`
   at once (a failure there is not fatal and is retried at the next sign-in).
   After the first OPAQUE success the vault records it (`account.pake_confirmed`,
   schema 8, plaintext 0/1, documented as plaintext metadata) and the device
   **never falls back** for that account. Credential change and deletion use
   OPAQUE when the flag is set, the legacy proof otherwise.
3. **Cutoff.** `HAVENKEYS_LEGACY_AUTH_UNTIL` (RFC 3339) is set by the
   operator to the deploy date + **30 days**; unset means no cutoff (a
   self-hosted server that has not decided keeps working). After it, every
   legacy path answers **426 `upgrade_required`** — never 410, which old
   apps read as "account deleted" and wipe their local copy. Old apps show
   it as a generic sync failure; the account's data is kept. `admin
   list-accounts` shows `pake` or `legacy` per account so the operator can
   see who has not upgraded.
4. **Missed the window.** The account's data is untouched. The operator
   runs `admin allow-legacy-sign-in --email <e>`: for 24 hours that one
   account (only while it has no OPAQUE record) may use the legacy login
   again, so the owner's updated app signs in once and enrols; enrolment
   ends the grace.
5. **Remove the dead code** in a later release, once the cutoff has passed
   on the hosted server: the legacy routes and fields, `enroll`,
   `allow-legacy-sign-in`, `auth_verifier` and `legacy_grace_until`, the
   server's Argon2 verifier, and the clients' legacy calls and fallback
   (tracked in `docs/roadmap.md` §2).

During the window an unenrolled account is as exposed as today, a device
that has never done OPAQUE can be pushed onto the legacy path by someone in
the middle, and a wrong password costs two rate-limit attempts on such a
device — all recorded in the security review.

## 7. Docs and review

* **CLAUDE.md:** no rule is relaxed. Add an amendment noting that the auth
  key no longer leaves the device after the cutoff.
* **`security-review.md`** (required, auth + crypto): the handshake, the
  pending-login table, server-setup storage and backup, the migration window
  and anti-downgrade rule, unknown-email equivalence, the threat delta from
  §1 stated as in §1.
* **`threat-model.md`, `crypto.md`, `server-sync.md`, `deployment.md`**:
  replace "the client sends the auth key"; deployment adds
  `HAVENKEYS_LEGACY_AUTH_UNTIL` and the server-setup row to the backup notes.

## 8. Tests

* Core/client: enrol + sign in round trip; wrong master password or Secret
  Key fails on the client; a recorded `credentialFinalization` replayed fails;
  a `loginId` works once; the fallback is refused after one OPAQUE success.
* Server: unknown email and known email give answers of the same shape;
  `finish` after expiry is 401; rate limits charged at `start`; legacy routes
  426 after `HAVENKEYS_LEGACY_AUTH_UNTIL`; reauth proof from another account's session
  refused; no OPAQUE message or key appears in logs (`tests/no_logging.rs`).
* `scripts/test-server.sh` against real Postgres, plus a migration test:
  legacy account → updated client → enrolled → legacy refused (426) after
  the cutoff.

## 9. Decided

1. Window length: **30 days** (`HAVENKEYS_LEGACY_AUTH_UNTIL`).
2. Hybrid post-quantum TLS (`1password-parity.md` #10) ships **separately**,
   not in this release.
