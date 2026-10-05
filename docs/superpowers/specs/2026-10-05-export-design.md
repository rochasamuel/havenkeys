# Export and encrypted backup — design

Date: 2026-10-05. Status: approved in conversation.

## 1. Goal

Two needs, one feature:

* **Leave HavenKeys** — write the vault in a format other password managers
  import: Bitwarden JSON (rich) and a logins CSV (universal).
* **Back it up** — write an encrypted HavenKeys backup, protected by a
  password chosen at export time, that any HavenKeys install can restore
  even if the account, its Secret Key or the server is gone.

Both are desktop only (like import). Exporting only reads the local
replica, so it works offline; restoring is a write and needs the server.

## 2. Decisions taken

| Question | Decision |
|---|---|
| Purpose | Both: plaintext for other managers and an encrypted backup |
| Plaintext formats | Bitwarden JSON (unencrypted) + logins CSV (Chrome columns) |
| Backup key | A separate backup password, Argon2id → key → existing AEAD |
| Passkeys | In the encrypted backup only; never in a plaintext export |
| Restore into a non-empty vault | Items keep their IDs; an ID already in the vault is skipped, and so is the backup's Identity if the vault has one |
| Backup file format | Our own `.hkbackup`, built only from the core's existing primitives (approach A; `age` and Bitwarden's encrypted export rejected — scrypt instead of Argon2id / reimplementing Bitwarden's key scheme) |

## 3. CLAUDE.md amendment

The passkeys amendment says the private key is "generated, stored and used
only in the Rust core". Add:

> Amended on 2026-10-05 by `docs/superpowers/specs/2026-10-05-export-design.md`:
> the desktop can export the vault. Plaintext exports (Bitwarden JSON, CSV)
> follow §38 and never contain passkey private keys. An encrypted HavenKeys
> backup, sealed under a backup password the user chooses (Argon2id + the
> vault's AEAD), carries every item including passkeys, and restoring it
> needs that password. Every export asks for the master password again.

§38 itself already allows export with explicit confirmation.

## 4. What each format contains

| | Backup | Bitwarden JSON | CSV |
|---|---|---|---|
| Logins: title, URLs, username, password, notes | ✓ | ✓ | ✓ |
| TOTP | ✓ (config) | ✓ `login.totp` as `otpauth://` | ✓ `otpauth://` in a `totp` column |
| Custom fields | ✓ | ✓ `fields` (hidden for concealed values) | — |
| Password history | ✓ | ✓ `passwordHistory` | — |
| Passkeys | ✓ | **never** | **never** |
| Android app bindings | ✓ | ✓ `androidapp://<package>` URIs | — |
| Secure notes | ✓ | ✓ type 2 | — |
| Cards | ✓ | ✓ type 3 | — |
| Identity | ✓ | ✓ type 4 | — |
| Item IDs, created/updated timestamps | ✓ | ✓ `id`, `creationDate`, `revisionDate` | — |

What a format cannot carry is counted, never silently dropped: the export
report lists e.g. "12 passkeys, 3 cards, 1 identity not included". The
counts are shown in the confirmation **before** the file is written.

CSV columns: `name,url,username,password,note,totp`. Chrome reads the first
five and ignores `totp`; Bitwarden, KeePassXC and our own importer read
it. A login with several URLs gets the first one in `url` and the rest in
`note` as "Website: …" lines (the reverse of import's `collect_urls`).

Bitwarden URI `match`: `Domain` → `null` (default, base domain), `Origin`
→ 1 (host), `Exact` → 3 (exact). Folders and collections are empty.

## 5. Backup format (`.hkbackup`)

```text
offset  size  field
0       8     magic  "HKBACKUP"
8       1     format version (1)
9       1     KDF algorithm (1 = Argon2id v1.3)
10      4     memory KiB      (u32 LE)
14      4     iterations      (u32 LE)
18      4     parallelism     (u32 LE)
22      16    salt (random, crypto::fill_random)
38      …     EncryptedBlob (existing crypto::blob format: version,
              algorithm, nonce, ciphertext+tag)
```

* Key: `derive_master_key(backup_password, params)` with a fresh
  `KdfParams::generate()` (today's defaults: 128 MiB, 4 iterations, 4
  lanes). The backup password is not combined with a Secret Key: the point
  of the backup is to restore without the account.
* Sealing: `blob::seal(key, ctx, payload)` with a new `Purpose::Backup`
  (label `backup`). The associated data covers the 38-byte header, so
  lowering the KDF cost or changing the version breaks authentication. The
  context needs no vault or item ID (a backup restores into any vault).
* Payload: `serde_json` of
  ```text
  { "version": 1, "exportedAt": <ms>, "items": [ BackupItem… ] }
  BackupItem = { id, type, createdAt, updatedAt, overview, details }
  ```
  where `overview` and `details` are the existing `ItemOverview` /
  `ItemDetails` serialisations, so a backup is exactly what the vault
  holds, passkeys included.
* Compression: none (no length side channel; files stay small).
* `MAX_BLOB_LEN` is 8 MB; a backup may exceed it. The backup uses its own
  limit (64 MB file, as import) and calls the AEAD through a
  backup-specific `seal_large`/`open_large` in `blob.rs` that is the same
  construction with a different length cap — not a new primitive.

### 5.1 Reading a backup (hostile input)

In order, refusing at the first failure with a message that never echoes
content:

1. File ≤ 64 MB (metadata, then what is read), magic matches.
2. Version known (1), else "This backup was made by a newer HavenKeys."
3. KDF algorithm known and `KdfParams::validate` passes (the existing
   min/max bounds stop a backup from asking for 100 GB of memory).
4. Derive, open. Any AEAD failure → "Wrong backup password, or the file
   is damaged." (one message: the two cannot be told apart).
5. Decrypted payload ≤ 64 MB, parsed with `serde_json` into the closed
   structs (`deny_unknown_fields` on the envelope), ≤ `MAX_ITEMS` items.
6. Every item goes through `build_item` validation in `stage_import`; an
   item that fails counts in `failed`. Passkeys are checked like a created
   one (key parses, RP ID valid).

## 6. Core (`crates/havenkeys-core/src/export/`)

* `mod.rs`: `ExportFormat` (closed enum: `Backup`, `BitwardenJson`,
  `Csv`), `ExportSummary` (counts per type and per left-out kind — no
  titles, no values), `summarize(vault, format)` and
  `export(vault, format, backup_password: Option<&SecretString>)
  -> Result<Zeroizing<Vec<u8>>>`.
* `bitwarden.rs`, `csv.rs`: writers. CSV through the `csv` crate (already
  a dependency) so quoting is correct; values that start with `=`, `+`,
  `-` or `@` are written as-is (the reader is another password manager,
  not a spreadsheet — stated in the export dialog: "don't open this file
  in a spreadsheet").
* `backup.rs`: `write_backup` and `read_backup -> Parsed`, so restore
  reuses the import pipeline.
* `import::ImportSource` gains `HavenKeysBackup`; `import::parse` for it
  needs the password, so the desktop calls `export::backup::read_backup`
  directly and then `stage_import` as usual.
* `stage_import` gains a "keep IDs" mode used only by restore: item IDs
  and timestamps come from the backup; an ID already present is skipped
  and counted in a new `skipped_existing`; the backup's Identity is
  skipped (`skipped_existing`) when the vault already has one, and is
  otherwise written under this vault's derived identity ID
  (`derive_identity_item_id`) rather than the backup's.
* Plaintext is built in `Zeroizing` buffers; the decrypted item list is
  dropped as soon as the file bytes exist. `Debug` stays redacted.

### 6.1 Re-authentication

`Vault::verify_master_password(password, secret_key) -> Result<()>`:
derives the KEK from the local header's KDF params and unwraps the vault
key; success only if it unwraps and equals the session key. Slow
(Argon2id), so the desktop takes a ticket (header copy) under the lock and
derives outside it, like `BundleTicket`. Wrong password → `UnlockFailed`.
Not rate-limited beyond Argon2id's cost (same as unlock).

## 7. Desktop

### 7.1 Commands (`apps/desktop/src-tauri/src/export.rs`)

* `export_summary(format) -> ExportSummary` — requires unlocked.
* `export_file(format, master_password, backup_password?)
  -> Option<ExportResult>` — requires unlocked; verifies the master
  password; for `Backup` checks the backup password (§7.3); opens the
  native save dialog (title and filter from Rust, default name
  `havenkeys-export-YYYY-MM-DD.json|.csv|.hkbackup`); writes the file
  created with mode 0600 on Unix (write to a temp file in the same
  directory, then rename); returns the file name and counts. `None` if
  the dialog is cancelled. The renderer never supplies a path.
* Restore goes through `import_file(HavenKeysBackup, backup_password)`:
  requires unlocked and online, picker filtered on `.hkbackup`, then
  `read_backup` → `stage_import` (keep IDs) → `push_batches`. No "delete
  the file" offer — the backup is encrypted and meant to be kept.

Passwords arrive as `SecretString` in the command arguments and are never
logged or echoed. Every command is added to the capability allowlist.

### 7.2 UI — Settings → Export (`ExportSection`)

1. Radio list: *HavenKeys backup (encrypted, recommended)*, *Bitwarden
   JSON*, *CSV (logins only)*, each with one line on what it is for.
2. The counts from `export_summary`, including what is left out.
3. Plaintext formats: a danger notice — "Export contains all passwords in
   plaintext. Anyone who gets this file can read every password. Delete it
   once you've imported it elsewhere, and don't open it in a spreadsheet."
   — and a checkbox "I understand" that enables the button.
4. Backup: backup password twice, with a hint to use the generator, and a
   warning that a lost backup password means a lost backup.
5. Master password field, then "Export…" opens the save dialog.
6. Result: "Exported N items to <file name>." For plaintext, a reminder to
   delete it.

Restore: *HavenKeys backup* added to the source list in `ImportSection`,
with a backup password field; the report shows `skipped_existing`. All
new strings get pt-BR translations. Password fields are cleared on
success, failure and unmount.

### 7.3 Backup password rules

Checked in Rust: same length bounds as the master password (10 to the
existing maximum); must differ from the master password (compared in Rust
after verification, constant-time). The two fields must match (UI only).

## 8. Security analysis

* **Unattended unlocked desktop**: the master password is required again,
  so walking up to an unlocked screen does not yield a file of every
  password.
* **Stolen backup**: protected by Argon2id at the default cost and AES-256-GCM;
  strength is the backup password's. KDF parameters are authenticated, so
  an attacker cannot rewrite them to make the user re-derive cheaply.
* **Malicious backup** (someone hands the user a crafted file): every
  limit in §5.1; restored items go through normal validation and are
  written with the vault's own keys; existing items are never overwritten.
  Restored logins' URL rules are as written in the file — the same trust
  as an import the user chose.
* **Plaintext file on disk**: unavoidable for migration; 0600, explicit
  confirmation, warning, no passkeys. Not deleted by us (the user needs
  it to import elsewhere).
* **Renderer compromise**: it can call `export_file` only with the master
  password, and cannot choose the path; the save dialog is native.
* **Leaks**: nothing logged; errors are fixed strings; the summary holds
  counts only; the extension and native-messaging protocol gain nothing
  (`export_vault` stays rejected there).
* **Memory**: best-effort zeroisation as elsewhere (docs/security-model.md).

## 9. Testing

Core:
* Backup round trip with every item type, custom fields, history, app
  bindings and a passkey that still signs after restore.
* Restore into a vault holding some of the same IDs and an Identity →
  those skipped, counts right, nothing overwritten.
* Refusals: wrong password, a flipped bit in the header, in the KDF
  params, in the ciphertext; truncated file; bad magic; version 2; KDF
  params out of bounds; payload with an unknown field; too many items.
* Bitwarden JSON and CSV exports re-imported with our own importers give
  the same logins (title, URLs, username, password, TOTP, notes) and the
  counted left-outs.
* No plaintext export contains passkey key material (search the output
  for every passkey's key bytes, base64 and base64url).
* CSV quoting: commas, quotes, newlines, leading `=`.
* `verify_master_password` accepts the right password, refuses a wrong one.
* Deterministic fuzz: mutated backups never panic.

Desktop:
* `ExportSection`: the confirmation gates the button; passwords cleared
  after each attempt; `exportFile` called with the chosen format.
* `ImportSection` restore path calls `importFile` with the backup source.
* Tauri crate builds; commands are in the capability file.

## 10. Documentation

* CLAUDE.md amendment (§3).
* `docs/crypto.md`: the `.hkbackup` format and key derivation.
* `docs/security-model.md` and `docs/threat-model.md`: export, backup,
  restore.
* `docs/security-review.md`: an entry for plaintext exports on disk.
* README: exporting and restoring.
