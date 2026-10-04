# Signing in a new desktop from the phone — Design

Status: proposed, 2026-10-03.
Builds on `2026-09-20-server-authoritative-vault-design.md` (accounts,
sessions, devices) and `2026-10-01-android-app-design.md` (the phone app).
Amends CLAUDE.md (a note, like the earlier ones), `docs/threat-model.md`
and `docs/security-model.md`.

> This software has not undergone an independent security audit.

## 1. Goal

Signing in a new desktop today needs the Emergency Kit (server, email,
Secret Key) and the master password. With this design, the desktop shows a
QR code, the user scans it with the phone (unlocked, online), checks the
device's name and location, and taps Allow. The desktop opens the vault
without the kit and without the master password.

Only the first sign-in changes. From then on the desktop is an ordinary
device: it unlocks with the master password (and the Secret Key it was
given), and can be revoked in Devices.

Out of scope: a new phone approved by a phone or by the desktop, approving
from the desktop, approving by anything but a scanned code.

## 2. The trust decision

Today the master password is always the last barrier: the server and the
Secret Key together cannot open a vault. Here an unlocked phone, plus the
user's Allow, is enough to bring a new device into the vault.

That is accepted, as in 1Password's and Bitwarden's device approval,
because:

1. The phone must be unlocked and online, and Allow asks for the phone's
   biometrics or device credential again.
2. The key material is sealed to a public key that travels from the
   desktop's screen to the phone's camera. The server relays only
   ciphertext and cannot swap the recipient.
3. The confirmation names the device and where the server saw it, and the
   code expires in 2 minutes and works once.

The remaining risk is a person being talked into scanning an attacker's
code and tapping Allow (§7). It is documented as such.

## 3. The flow

### 3.1 Desktop: start

On the Welcome screen, next to "Sign in with Emergency Kit", a new choice:
"Sign in with your phone". The user types the server address only; the
phone says which account it is.

1. Rust generates an HPKE key pair (§4) and a 32-byte random
   `claim_secret`. Both live in memory only, in `HavenClient`.
2. `POST /v1/pairings` with `{device_id, device_name, public_key,
   claim_hash = SHA-256(claim_secret)}`. `device_id` is the desktop's own
   persistent device id, the one a login would register. No
   authentication.
3. The server stores the request (§5.1) and answers `{pairing_id,
   expires_at}`. `pairing_id` is 128 random bits, base64url.
4. The desktop shows the code for
   `havenkeys://pair/v1?server=<url>&id=<pairing_id>&pk=<base64url public key>`
   and the time left. At expiry it offers "New code", which starts again
   from step 1 with a new key pair.
5. Every 2 seconds, `POST /v1/pairings/{id}/claim` with `claim_secret`.

The QR code is encoded in Rust (`qrcode`, already a desktop dependency)
into a module grid, as the Emergency Kit's is, and React draws the grid;
React gets the grid and the states, never a key.

### 3.2 Phone: scan and confirm

Entry: Settings → "Sign in a new device". The screen is the camera
(`KitScanner`).

1. Each frame goes to Rust (`scan_pairing`). Rust keeps only a link that
   parses as `havenkeys://pair/v1`. Any other code is dropped.
2. Rust refuses a link whose server is not this account's server
   ("This code is for another server."), and refuses when the vault is
   locked or offline.
3. `GET /v1/pairings/{id}` with the phone's session returns
   `{device_name, ip, location, created_at, expires_at}`.
4. The confirmation sheet:

   ```text
   Sign in a new device?
   Desktop – Linux
   São Paulo, BR · 187.x.x.x · just now

   [ Deny ]                    [ Allow ]
   ```

   With no location database on the server, the line shows the IP only.
5. Allow asks `BiometricGate` (biometrics or the device credential). Then
   Rust seals the envelope (§4) to the link's `pk` and calls
   `POST /v1/pairings/{id}/approve` with `{envelope}`.
6. Deny calls `POST /v1/pairings/{id}/deny`.

The phone keeps the link and the pairing's details only while the sheet is
open.

### 3.3 Server: approve

In one transaction:

1. The pairing exists, is `pending` and not expired. Otherwise:
   `pairing_gone` (one error for unknown, expired, used, denied).
2. The session's account is recorded on the pairing. A pairing that
   another account already looked at or approved is refused (§5.2).
3. The request's `device_id` is registered as a login does
   (`register_device`: refused if it belongs to another account or was
   revoked; the 64-device cap, `MAX_DEVICES_PER_ACCOUNT`), with the
   request's `device_name`, marked `approved_by = <phone's device id>`.
4. A session token is issued for that device (24 h,
   `SESSION_TTL_HOURS`). The token itself is kept on the pairing only
   until the claim; its hash goes in `sessions` as for any login.
5. The pairing becomes `approved`, holding the envelope and the token.

### 3.4 Desktop: claim and finish

`claim` with the right `claim_secret` (constant-time compare of hashes):

* `pending` → `{state: "waiting"}`
* `denied` → `{state: "denied"}`, and the pairing is deleted
* `approved` → `{state: "approved", token, expires_at, account_id,
  vault_id, envelope}`, and the pairing is deleted: a second claim gets
  `pairing_gone`. The desktop does not take `account_id` or `vault_id`
  from this answer on trust: both must equal the ones inside the envelope.
* wrong secret, unknown or expired → `pairing_gone`

Then, in Rust (`finish_pairing`), as `sign_in` does from step 5 on:

1. Open the envelope (§4). Any failure: "The sign-in could not be
   completed. Ask for a new code." Nothing is kept.
2. `GET /v1/vault/header` with the new session. Check the header's
   attestation with the envelope's vault key (`verify_header`) and that the
   header's vault id and account match the envelope's.
3. Create the local vault from the header and the vault key (a sibling of
   `prepare_sign_in` that takes the vault key instead of the password),
   store the `AccountRecord` and the Secret Key (`set_secret_key`).
4. Go online with the session, sync, ensure the identity, as after a
   sign-in. The vault is open.

The next unlock is the ordinary one: master password, with the stored
Secret Key, against the local header.

## 4. The envelope

HPKE (RFC 9180), base mode, suite DHKEM(X25519, HKDF-SHA256),
HKDF-SHA256, AES-256-GCM, through the RustCrypto `hpke` crate. No other
construction is assembled by hand. Before it is added, the crate's
maintenance, licence and audit status are checked and `cargo audit` /
`cargo deny` run (CLAUDE.md §48); if it does not pass, the plan stops
and asks rather than substituting another construction.

* `info` = `"havenkeys/pair/v1" ‖ 0x00 ‖ server_url ‖ 0x00 ‖ pairing_id`,
  so an envelope cannot be replayed to another pairing or server.
* Plaintext: a versioned, length-checked binary record
  `{version = 1, account_id (16), vault_id (16), email, secret_key,
  vault_key (32)}`. Held in `Zeroizing` buffers and `SecretString` on both
  sides, never logged, never in `Debug` output.
* Stored and sent as `{version, suite_id, encapsulated_key, ciphertext}`,
  base64url in JSON, under a size limit (`MAX_PAIRING_ENVELOPE_BYTES`,
  4 KiB).

The vault key is sent rather than the data key so the desktop can build
its local vault exactly as a sign-in does and verify the header itself.
An unlocked vault keeps only the data key today, so the core session also
keeps the vault key while unlocked (a `Key256`, zeroized on lock like the
data key, and never returned by any API). The envelope is sealed inside
`VaultService`, so the vault key never leaves the core.

## 5. Server

### 5.1 The `pairings` table

```text
id              text primary key   -- 128-bit random, base64url
state           text               -- pending | approved | denied | claimed
device_name     text               -- ≤ MAX_DEVICE_NAME_CHARS
public_key      bytea              -- 32 bytes
claim_hash      bytea              -- SHA-256(claim_secret)
ip              text               -- as `client_ip` gives it
location        text null          -- "São Paulo, BR", from the local database
created_at      timestamptz
expires_at      timestamptz        -- created_at + 2 minutes
account_id      uuid null          -- set by the first details/approve
device_id       uuid               -- the desktop's own id, from create
envelope        bytea null
token           text null          -- cleared at claim
token_expires_at timestamptz null
```

`devices` gains `approved_by uuid null`, the phone's device id, for the
Devices list.

The server has no periodic task, so each create first deletes every
pairing older than 10 minutes, whatever its state; claim and the session
routes treat an expired row as gone.

### 5.2 Routes

| Route | Auth | Does |
|---|---|---|
| `POST /v1/pairings` | none | create; rate-limited per IP |
| `GET /v1/pairings/{id}` | session | details; binds the pairing to the account |
| `POST /v1/pairings/{id}/approve` | session | §3.3 |
| `POST /v1/pairings/{id}/deny` | session | state `denied` |
| `POST /v1/pairings/{id}/claim` | `claim_secret` | §3.4 |

* A pairing is bound to the first account whose session reads or approves
  it; any other account gets `pairing_gone`.
* `create`: at most 10 per IP per 10 minutes and 3 pending per IP;
  over that, `429`.
* `claim`: a wrong secret counts against the caller's IP like a failed
  login (`rate_limit::record_failure` on the IP key), and a blocked IP is
  refused before anything is read.
* A claimed or denied pairing is kept as `claimed` (envelope and token
  cleared) until it ages out, so it still counts against its IP's limit.
* All bodies under strict size limits; unknown fields refused.

### 5.3 Location

Optional environment variable
`HAVENKEYS_GEOIP_DATABASE=/path/to/dbip-city-lite.mmdb` (the server is
configured from the environment), read
with the `maxminddb` crate at start-up. The lookup is local; no third
party is called. Without it, `location` is null. The IP is the server's
own (`client_ip`, which honours `X-Forwarded-For` only with
`trust_forwarded_for`), never one the desktop reports.

## 6. Components

* **core** `pairing` module: build and parse the `pair/v1` link; seal
  and open the envelope; the plaintext record's encoding. Pure, fuzzable.
* **sync-client**: the five calls, typed.
* **client**:
  * desktop side: `start_pairing(server) → link`,
    `poll_pairing() → Waiting | Denied | Expired | Approved`,
    `finish_pairing()`, `cancel_pairing()`;
  * phone side: `pairing_details(link)`, `approve_pairing(link)`,
    `deny_pairing(link)`.
* **server**: migration, routes, rate limits, cleanup, optional location.
* **desktop**: Tauri commands `pairing_start`, `pairing_poll`,
  `pairing_cancel` (allowlisted); `PhoneSignInPanel` in
  `WelcomeScreen.tsx` with the QR SVG, the countdown, "New code" and "Use
  Emergency Kit".
* **mobile** (uniffi): `scan_pairing(frame) → PairingRequest?`,
  `approve_pairing`, `deny_pairing`.
* **Android**: Settings entry, the scan screen, the confirmation sheet
  (Allow behind `BiometricGate`), strings in English and pt-BR.
* **Devices** (phone and desktop): a device registered by a pairing says
  "Approved by <phone's name>".

## 7. Threats

| Threat | Defence |
|---|---|
| Hostile or compromised server | Sees only the envelope, sealed to a key it did not choose; it can lie about name and location but cannot open or redirect the keys. |
| A code from an attacker ("scan this…") | Name, IP and location shown; Allow needs biometrics; 2-minute, single-use code. Residual, documented. |
| Guessed or leaked `pairing_id` | Details and approve need a session on the account; claim needs `claim_secret`, which is not in the code. |
| Replay | Deleted on claim, deny and expiry; `info` binds envelope to server and pairing. |
| Abuse of the open endpoints | Per-IP limits, pending cap, size limits, generic errors. |
| Compromised desktop renderer | Keys stay in Rust; React gets the SVG and states. |
| Accidental disclosure | Envelope, Secret Key, vault key and token zeroized after use, never logged or put in errors. |
| Locked or offline phone | Rust refuses before calling the server. |

## 8. Errors (user-facing)

* "This code has expired. Ask the new device for a new one."
* "This code is for another server."
* "Unlock HavenKeys and connect to your server to approve a device."
* Desktop after Deny: "The sign-in was denied on your phone."
* Desktop, any other failure: "The sign-in could not be completed. Ask
  for a new code."

## 9. Tests

* **core**: envelope round trip; tampered ciphertext, encapsulated key and
  `info`; wrong key; unknown version or suite; malformed and oversize
  links and envelopes (and a fuzz target for the link and the envelope).
* **server** (Postgres): create and its limits; expiry; details and
  approve from another account; approve twice; deny; claim with a wrong
  secret; claim once only; the device cap; a revoked approver's session;
  the claimed token works and is the only session for that device.
* **client round trip**: phone approves, desktop claims, opens, syncs,
  locks, unlocks with the master password; a denied and an expired
  pairing leave nothing on the desktop.
* **mobile**: `scan_pairing` keeps only `pair/v1` links; another server is
  refused; locked refuses.
* **Android**: the sheet shows the details; Allow requires the gate; Deny
  calls deny.
* **desktop**: panel states (waiting, expired → new code, denied,
  approved).

## 10. Documentation

`docs/threat-model.md` (§2's decision and §7), `docs/security-model.md`
(the new endpoints and the envelope), `docs/crypto.md` (HPKE suite and
`info`), `docs/architecture.md`, `docs/security-review.md` (a review of
this feature once built), and a CLAUDE.md amendment note.
