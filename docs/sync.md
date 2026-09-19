# Sync and the Secret Key

How one vault is shared between your devices without a HavenKeys server,
and how a new device proves it belongs to you. This is also the
specification the mobile app must follow.

> This software has not undergone an independent security audit.

## 1. Model

```text
 Computer A ──┐                               ┌── Computer B
              │  <folder you already sync>    │
              ├─►  HavenKeys/<vault id>/  ◄───┤
              │      header.json              │
 Phone (later)┘      devices/<A>.hks          └── ...
                     devices/<B>.hks
```

* You choose a folder that a sync service you already use keeps in step:
  OneDrive, Dropbox, Google Drive, iCloud Drive, Syncthing, a network share.
  HavenKeys never talks to that service. It only reads and writes files.
* Each device writes **only its own file** (`devices/<device id>.hks`), a
  full encrypted snapshot of its vault. It reads the other devices' files
  and merges them. Two devices never write the same file, so the sync
  service never has to resolve conflicts, and never creates
  "conflicted copy" files.
* The folder is treated as **untrusted storage**. Whoever runs the sync
  service, or anyone who gets into your cloud account, sees only ciphertext
  and a little metadata (§6). They cannot open it without your master
  password **and** your Secret Key.

## 2. Secret Key and Emergency Kit

Every new vault gets a **Secret Key**: 128 random bits from the OS CSPRNG,
shown as `H1-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`. That is Base32 (RFC 4648)
of the 16 bytes, plus 2 check characters for typo detection
(`docs/crypto.md`, "Secret Key").

```text
KEK = HKDF-SHA-256(ikm  = Argon2id(master password) ‖ Secret Key,
                   salt = vault ID,
                   info = "havenkeys/v2/kek")
```

* Every copy that is not on one of your devices needs both secrets: the
  sync folder, backups, a stolen `vault.sqlite3`. Offline guessing of the
  master password is useless without the 128-bit Secret Key.
* Each device stores the Secret Key in its own `device.json`, outside the
  vault database. Day to day you type only your master password.
* **A new device needs both.** Joining asks for the master password and the
  Secret Key. That is how a device "confirms your identity": no server is
  asked, and nothing is sent anywhere. Knowing both is the proof.
* The **Emergency Kit** is a printable page with the Secret Key, a QR code of
  it, and blank lines for the sync folder and, if you choose, the master
  password. It is shown right after a vault is created (continuing requires
  confirming it was saved), and at any time from Settings → Secret Key &
  sync while unlocked.
* Losing both the Emergency Kit and every device that holds the Secret Key
  means losing the vault. HavenKeys cannot recover it.

**QR payload** (for the mobile app to scan):

```text
havenkeys://kit/v1?vault=<vault id>&key=<Secret Key, H1 text form>
```

Vaults created before the Secret Key existed keep working with the master
password only (key scheme 1). Settings → Secret Key & sync → *Add a Secret
Key* upgrades them. It asks for the master password, re-wraps the vault key,
and leaves items untouched. Sync requires the upgrade.

## 3. Setting up

**First computer:** Settings → Secret Key & sync → *Choose sync folder…*.
HavenKeys creates `HavenKeys/<vault id>/` inside it and syncs.

**Another computer:** install HavenKeys and, on the first screen, choose
*Set up from a sync folder*. Pick the same folder and enter the master
password and the Secret Key. The device checks both by unlocking the
folder's header (§4), copies the vault, and from then on syncs itself.

**Sync runs** after unlock, a moment after each change (add, edit, delete,
import, a login saved from the browser), every minute while unlocked, and
on *Sync now*. It never runs while locked.

## 4. File formats (version 1)

All JSON is UTF-8. Base64 is standard, with padding. UUIDs are lowercase and
hyphenated. Times are Unix milliseconds.

### `header.json`

```json
{
  "header": {
    "format": 1,
    "vaultId": "…",
    "vaultFormat": 1,
    "keyScheme": "password_and_secret_key",
    "kdf": { "algorithm": "argon2id", "memory_kib": 131072, "iterations": 4, "parallelism": 4, "salt": "<base64>" },
    "wrappedVaultKey": "<base64 blob>",
    "revision": 3,
    "createdAt": 1700000000000
  },
  "attestation": "<base64 blob>"
}
```

* `wrappedVaultKey` is the vault key sealed under the KEK. It is the same
  encrypted blob format as everything else (`crypto.md`), with purpose
  `vault-key`.
* `attestation` = `seal(data key, purpose "sync-header", vault ID,
  canonical JSON of "header")`. The canonical JSON is the `header` object
  serialized with its fields in the order shown, and no whitespace.
* A new device parses the header, derives the KEK from the password and
  Secret Key, unwraps the vault key, derives the data key, and then
  **requires** the attestation to open and to equal the canonical header.
* An unlocked device adopts a folder header only if the attestation
  verifies, the key scheme is `password_and_secret_key`, and the header is
  newer: higher `revision`, ties broken by the larger `wrappedVaultKey`. This
  is how a master password change spreads. The other devices then need the
  new password at their next unlock. A header that is missing, older or
  invalid is replaced with the device's own.

### `devices/<device id>.hks`

One blob (`crypto.md` format): `seal(data key, purpose "sync-snapshot",
vault ID, item slot = device ID, JSON)`, where the JSON is:

```json
{
  "format": 1,
  "deviceId": "…",
  "writtenAt": 1700000000000,
  "items": [ { "id": "…", "overview": "<base64 blob>", "details": "<base64 blob>" } ],
  "tombstones": [ { "id": "…", "deletedAt": 1700000000000 } ]
}
```

* `overview` and `details` are the item's own encrypted blobs, exactly as
  stored in `vault.sqlite3`. Each is bound to the vault ID and its item ID.
  They are copied without re-encryption.
* A snapshot must open under the device ID in its **file name**, and its
  `deviceId` must match that name. A file copied under another name does not
  open.
* Files are written to a temporary name in the same folder, flushed, then
  renamed, so readers never see half a file.

Limits when reading: at most 32 device files, and 8 MiB per file. Unknown
fields anywhere are rejected (`deny_unknown_fields`).

## 5. Merge rules

For each item ID across all other devices' snapshots:

1. Every remote item version must authenticate: overview and details both
   open under the data key, the overview's ID matches, and the details type
   matches the overview type. Anything else is skipped and counted.
2. The remote candidate is the version with the newest `updatedAt` (from the
   decrypted overview), ties broken by device ID.
3. A remote deletion (the newest `deletedAt` for that ID) that is not older
   than the candidate wins over it.
4. Against the local vault:
   * local item: take the candidate only if it is **strictly newer**;
   * local tombstone: take the candidate only if it is newer than the
     deletion (an edit after a delete brings the item back);
   * nothing local: take it.
5. A remote deletion removes a local item if the deletion is not older than
   the local version. Tombstones are kept, so a deletion keeps propagating.

The merge is applied in one SQLite transaction. The device then writes its
own full snapshot.

## 6. Security properties and limitations

**Protected:**

* **Confidentiality:** everything in the folder, except `header.json`'s
  unlock parameters, is AES-256-GCM ciphertext under the vault's data key.
  Opening the vault needs the master password and the 128-bit Secret Key.
* **Integrity:** every snapshot, every item inside it, and the header
  attestation are authenticated. A forged or corrupted file is ignored, and
  a forged header is replaced. The tests cover tampering, files copied under
  another device's name, garbage, and forged headers
  (`crates/havenkeys-core/tests/sync.rs`).
* **No rollback through replays:** an old snapshot put back in the folder
  loses the merge, because its versions are older.
* **No downgrade:** a header without the Secret Key is never adopted.

**Not protected, or visible:**

* **Metadata:**
  * the vault ID, the KDF parameters and salt, the wrapped vault key and the
    header revision (in `header.json`);
  * the number of devices, the size of each snapshot (which tracks the
    number of items), and when each device last synced.
* **Denial of service:** anyone with access to the folder can delete files.
  Updates then stop reaching other devices, but nothing is lost locally.
* **Clocks decide conflicts** ("newest `updatedAt` wins"). A device whose
  clock runs far ahead wins concurrent edits. Deletions and edits carry the
  device's own time.
* **Any of your unlocked devices is trusted.** It holds the vault key, so it
  can change any item or the master password for all devices. Malware on
  one device can therefore reach the others through sync (`threat-model.md`
  §4, same-user malware, out of scope).
* **The vault key is never rotated** (`security-review.md` #8). Someone who
  once had a copy of the folder and both secrets can read later copies. A
  master password change does not change the Secret Key or the vault key.
* **The Secret Key on each device is stored in plain text** in `device.json`,
  as 1Password stores it on its devices. On a device, the master password
  alone protects the vault against someone who can read that device's
  files. The Secret Key protects every other copy.
* **Deletions are permanent records:** tombstones (item ID and time) are
  kept forever. They are small, and in the folder they are encrypted.

## 7. For the mobile app

* Scan the Emergency Kit QR code (or type the Secret Key), and have the user
  pick the synced folder, for example through the Files app or the storage
  access framework.
* Reuse `havenkeys-core` (Rust, for example through UniFFI) rather than
  reimplementing the formats: `sync::prepare_join`,
  `VaultService::{create_vault, sync}` and `sync::folder` are the whole
  flow, and the crypto stays in one audited implementation.
* Store the Secret Key in the platform keystore (iOS Keychain, Android
  Keystore), which is better than the desktop's `device.json`.
* Tune Argon2id on real phones. Parameters come from the header, so a phone
  must be able to run the desktop's parameters, or the vault needs lower
  ones (`crypto.md`, Argon2id parameters).
