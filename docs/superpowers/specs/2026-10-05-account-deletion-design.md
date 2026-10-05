# Account deletion (LGPD) — design

Date: 2026-10-05. Status: approved in conversation.

## 1. Goal

The author now operates a hosted `havenkeys-server` for other people, which
makes SAMUEL DA SILVA ROCHA DESENVOLVIMENTO DE SOFTWARE LTDA the *controlador*
of their personal data under the LGPD (Lei 13.709/2018). The data subject
must be able to exercise the right to deletion (art. 18, VI) without the
operator's help, and the deletion must be complete and verifiable.

This spec is sub-project **A** of LGPD compliance:

| Sub-project | Scope | This spec |
|---|---|---|
| A. Self-service total deletion | Delete the account from the app; nothing left on the server or the devices | Yes |
| B. Privacy policy and terms as controller | Legal bases, encarregado, international transfer, data-subject channel | Only the deletion section and the controller's name |
| C. Access and portability of server metadata | "What the server knows about me" | No |
| D. Operations | Log and Postgres-backup retention on Railway, sweeping expired sessions and login attempts, record of processing | No (this spec promises 30 days; D makes it true) |

## 2. Decisions taken

| Question | Decision |
|---|---|
| Timing | Immediate and irreversible; an encrypted backup is offered first |
| Where | In the desktop and Android apps; a public page on havenkeys.net explains how and gives an email for people who lost access (the operator then runs the admin CLI) |
| Web form | Rejected: it would put the master password in a web page, and the server accepts no browser origin (server design §7.6) |
| Other devices | They wipe themselves when the server says the account was deleted |
| How they learn it | An anonymous tombstone of the deleted account's session token hashes, kept 30 days |
| Proof required | Session **and** the current auth key (as for a credential change) **and** the account's email typed as confirmation |
| Contact | samuelsilv.rocha@gmail.com |
| Backup and log retention promised | Up to 30 days |

## 3. CLAUDE.md amendment

Under §1, after the 2026-09-27 amendment, add:

> Amended on 2026-10-05 by
> `docs/superpowers/specs/2026-10-05-account-deletion-design.md`: the author
> also operates a hosted `havenkeys-server` for other people, as controller
> under the LGPD. A user can delete their account from the desktop or
> Android app: the server erases everything it holds about the account in
> one transaction, keeping only anonymous session-token hashes for 30 days so
> the user's other devices learn of the deletion and wipe their local copy.

And replace "It is not a hosted service and there is no vendor account" in
§1 with "Anyone may run it; the author also runs one for other people. There
is no vendor account beyond an account on that server."

## 4. Server (`crates/havenkeys-server`)

### 4.1 Migration `0003_account_deletion.sql`

```sql
-- Session tokens of deleted accounts, so the account's other devices learn
-- of the deletion (docs/superpowers/specs/2026-10-05-account-deletion-design.md §4).
-- Nothing here identifies a person: no account id, no email, no device id.
CREATE TABLE deleted_sessions (
  token_hash BYTEA PRIMARY KEY CHECK (octet_length(token_hash) = 32),
  expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX deleted_sessions_by_expiry ON deleted_sessions (expires_at);
```

### 4.2 One erase function (`src/erase.rs`)

`erase_account(tx, account_id) -> Result<(), DbError>` runs inside a caller's
transaction, in this order:

1. `INSERT INTO deleted_sessions (token_hash, expires_at)
   SELECT token_hash, now() + interval '30 days' FROM sessions
   WHERE account_id = $1 ON CONFLICT DO NOTHING`.
2. Collect the account's device ids (needed for step 4, before the cascade
   removes them).
3. `DELETE FROM login_attempts WHERE key = 'acct:' || $1`.
4. `DELETE FROM pairings WHERE account_id = $1 OR device_id = ANY($devices)`
   — pending pairings have no `account_id` yet but carry a device id, name
   and IP.
5. `DELETE FROM accounts WHERE id = $1`. The existing `ON DELETE CASCADE`
   removes `vaults`, `items`, `devices`, `sessions` and linked `pairings`.

IP-keyed `login_attempts` rows (`ip:<addr>`) are not tied to an account and
are left to sub-project D's sweep.

Both callers use it, so deletion from the app and deletion by the operator
leave exactly the same state:

* `admin delete-account --email` looks up the id, opens a transaction, calls
  `erase_account`, commits.
* `POST /v1/account/delete` (below).

### 4.3 `POST /v1/account/delete` (`src/routes/account.rs`)

Body (`deny_unknown_fields`):

```json
{ "currentAuthKey": "<b64>", "email": "user@example.com" }
```

1. `Session` extractor (bearer token).
2. Normalize `email`; it must equal the account's `email_normalized`, else
   `400 invalid_request` ("the email does not match this account"). This is
   a typo guard, not a security check.
3. Rate limit and verify `currentAuthKey` exactly like `change_credentials`:
   checked before the transaction, a failure is recorded and returns `401`.
   A stolen token alone therefore cannot delete the account.
4. Transaction: `SELECT auth_verifier ... FOR UPDATE`, compare with the value
   verified in step 3 (a concurrent credential change makes this `409`),
   `erase_account`, commit.
5. `204 No Content`. Log one line: `account deleted`, with `account_id`
   only.

### 4.4 Telling the other devices

In the `Session` extractor, when the main query finds no live session, run
`SELECT 1 FROM deleted_sessions WHERE token_hash = $1 AND expires_at > now()`.
If found, reject with a new `ApiError::AccountDeleted` → `410 Gone`,
`{"error":"account_deleted"}`; otherwise `401` as today. Only a holder of
the token can see the `410`, so nobody can probe whether an account existed.

A device whose session expired before it came back online, or that stays
offline for more than 30 days, sees only the ordinary signed-out state; the
user then removes the device as today. This is documented as a limitation.

### 4.5 Sweeping tombstones

The server has no periodic task today. `main` spawns one `tokio` interval
(every 24 h, first run at startup) that runs
`DELETE FROM deleted_sessions WHERE expires_at <= now()`. Errors are logged
by kind only and the loop continues. Sub-project D may add other sweeps to
this task.

## 5. Client core (`crates/havenkeys-client/src/deletion.rs`)

### 5.1 `HavenClient::delete_account(confirmation, master_password)`

1. Unlocked and online, checked in Rust (the shell is not trusted to gate
   this — same reasoning as `remove_device`).
2. `confirms(&confirmation, &account.email)` (reuse from `removal.rs`).
3. Derive the current auth key from the master password and the Secret Key
   the same way the credential change does; the master password never leaves
   Rust, and the derived key is zeroized after the request.
4. Call `POST /v1/account/delete` through `havenkeys-sync-client`.
5. On any error, nothing local changes; the error maps to a fixed,
   secret-free message (`wrong_password`, `offline`, `rate_limited`,
   `generic`).
6. On `204`, run `wipe_local()`.

### 5.2 `wipe_local()`

1. Write a `pending-deletion` marker file next to the vault (contains the
   account id only).
2. Lock the vault, forget the server and session, replace the store with an
   in-memory one (closes the connection).
3. **Delete** the vault file and its SQLite sidecars (`-wal`, `-shm`) —
   unlike `remove_device`, which renames it.
4. `KeyStore::delete(account)` (Secret Key), and on Android the activity
   record and per-account settings.
5. Delete the marker; emit the "account deleted" event; the app returns to
   first run.

On startup, if the marker exists, `wipe_local()` resumes from step 2. A
server-confirmed deletion therefore never leaves an orphan replica because
one step failed once.

Vault files previously set aside by `remove_device` are **not** touched:
they may belong to another account or be the only copy of an old server.
The deletion screen says so when any exist.

### 5.3 `410 account_deleted` during sync

`havenkeys-sync-client` maps `410` with `account_deleted` to a new
`SyncError::AccountDeleted`. The client runs `wipe_local()` without asking —
the account no longer exists — and the next screen says "This account was
deleted" instead of an empty sign-in.

### 5.4 Mobile bindings (`crates/havenkeys-mobile`)

Expose `delete_account(confirmation, master_password)` and the
`account_deleted` event over the existing UniFFI surface. The Android key
store implementation already supports `delete`.

## 6. Desktop (`apps/desktop`)

Tauri command `delete_account { confirmation, masterPassword }` (allowlisted
in the capability file), calling 5.1. Settings → Account gains a danger
zone, "Delete account and all data":

1. **Explain**: what disappears (the vault on the server, every device, this
   computer's copy), that it is irreversible, and that neither the user nor
   the operator can recover it. Primary button: **Make an encrypted backup
   first** (opens the existing HavenKeys backup export). Secondary:
   "Continue without a backup". Mentions set-aside vault files if any.
2. **Confirm**: type the account email and the master password; the button
   stays disabled until the email matches. Password field cleared on
   failure and unmount.
3. **Done**: "Account deleted." Back to first run.

PT-BR and EN strings.

## 7. Android (`apps/android`)

Settings → Account, same three steps. Biometric prompt before step 2. No
export on Android, so step 1 says "To keep a copy, export from the desktop
first", with "Delete anyway". Handle the `account_deleted` event from 5.3
with the "This account was deleted" screen.

## 8. Extension

It stores no account data (only the menu preference), so nothing to erase.
It sees the desktop's "no account" status as it does after `remove_device`.

## 9. Website (`apps/web`)

* New page `/delete-account` (PT-BR and EN), linked from the footer and the
  privacy policy: how to delete from the desktop and Android apps, what is
  erased and when, and "Lost access to your account? Email
  samuelsilv.rocha@gmail.com from the account's address." This is the public
  deletion URL Google Play asks for.
* Privacy policy:
  * Names the controller: SAMUEL DA SILVA ROCHA DESENVOLVIMENTO DE SOFTWARE
    LTDA, contact samuelsilv.rocha@gmail.com.
  * Replaces "Não operamos nenhum servidor hospedado e não temos acesso ao
    seu" (and the EN equivalent) with a statement that the developers also
    operate a server for others, still unable to decrypt vaults.
  * New section "Exclusão da conta": immediately — vault, items, devices,
    sessions, email and login-attempt counters; within 30 days — the
    anonymous session tombstones; within 30 days — database backups and
    server logs.
  * "Atualizada em" bumped.

The rest of the policy (legal bases, encarregado, international transfer)
belongs to sub-project B.

## 10. Security analysis

* **Stolen session token**: cannot delete; the auth key is also required,
  and attempts count toward the login rate limit.
* **Malicious or compromised server** can send `410` and make devices wipe
  their replica. It gains nothing it does not already have: it holds the
  vault ciphertext and can delete it anyway. The user's protection against
  that is the encrypted backup, which the flow offers first.
* **Network attacker**: TLS; the `410` is only honoured on an authenticated
  response from the configured server.
* **Tombstone**: SHA-256 hashes of random tokens, with an expiry. Not
  personal data on its own; expires in 30 days.
* **Probe for existence**: `410` requires the token; `auth/params` keeps its
  uniform answer for unknown emails.
* **Residual data** (documented, not hidden): Postgres backups and platform
  logs until their retention ends (≤ 30 days, made true by D); IP-keyed
  login-attempt rows; set-aside local vault files from earlier device
  removals; copies the user exported themselves.

## 11. Testing

Server (Postgres integration tests, same harness as `tests/admin.rs`):

* After `POST /v1/account/delete`, no row in **any** table of the schema
  references the account id, email, device ids or pairing IPs. The test
  enumerates tables from `information_schema`, so a future table without a
  cascade fails it.
* Right token, wrong auth key → `401`, nothing deleted, failure recorded.
* Rate-limited account → `429`, nothing deleted.
* Mismatched email → `400`, nothing deleted.
* Another session of the deleted account → `410 account_deleted`; an
  unknown token → `401`; a tombstone past `expires_at` → `401`.
* Sweep removes expired tombstones only.
* `admin delete-account` leaves the same state as the route.
* Concurrent credential change during deletion → `409`, nothing deleted.

Client (`stub_server`):

* Locked → refused; offline → refused; wrong confirmation → refused.
* Server error → vault file, Secret Key and settings intact.
* `204` → vault file and sidecars gone, `KeyStore::delete` called, first-run
  state.
* Marker present at startup → wipe resumes and completes.
* `410` during sync → wipe.
* A set-aside file from `remove_device` survives.
* Errors contain no password, email or key material.

Desktop and Android: UI tests for the three steps, button gating on the
email, password cleared on failure.

## 12. Documentation

* `CLAUDE.md` amendment (§3).
* `docs/server-sync.md`: the route, the tombstone, `410`.
* `docs/threat-model.md`: malicious-server wipe (§10) and residual data.
* `docs/security-model.md`: what deletion erases and what it does not.
* `docs/deployment.md`: the sweep task; operator deletion via the CLI now
  equals in-app deletion.
* `docs/security-review.md`: entry for this feature.
