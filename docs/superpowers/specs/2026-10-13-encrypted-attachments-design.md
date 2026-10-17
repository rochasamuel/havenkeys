# Encrypted attachments — design

Date: 2026-10-13, revised 2026-10-17. Status: **approved 2026-10-17.**

Attach small files — a photo of an RG or CNH, a receipt, a PDF — to any item,
sealed with the vault's existing AES-256-GCM blob, **at most 5 MiB per file**,
so the server stores ciphertext it cannot open. Expands `docs/ideas.md` #4
and parity #5 (`docs/1password-parity.md`).

The 2026-10-17 revision replaced chunked uploads with one blob per file (the
5 MiB cap makes chunks unnecessary). It also closed gaps the first draft left
open: orphaned uploads, older clients deleting attachments, the quota pushing
out passwords, trash and purge, export and backup, image preview safety, and
the device cache.

## 1. Goal

Documents belong with the item they prove, and they are as sensitive as a
password. So:

* a file attached to an item is sealed under the vault data key, bound to its
  vault, item and attachment id, and byte-for-byte opaque to the server;
* the list of attachments (names, sizes, types) is inside the item's sealed
  overview, so it is on every replica and visible offline;
* the bytes are fetched on demand and cached on the device **as ciphertext**;
* no new cryptography: one new `Purpose` arm on the existing v1 blob.

Non-goals (v1): server-side previews or thumbnails; editing a file in place;
opening a file in another app through a plaintext temporary file; attachments
in the browser extension; attachments in Bitwarden JSON/CSV exports or
imports; files larger than 5 MiB.

Anti-goals — the feature fails if any of these happen, even if it "works":

* a password or other item edit is refused because attachments filled the
  vault;
* an older HavenKeys build silently deletes a user's attachments;
* a plaintext copy of a file is left on disk that the user did not choose to
  save;
* a hostile server can make the app show the wrong file, a truncated file, or
  another item's file without an error.

## 2. What exists today (cited)

* `crates/havenkeys-core/src/crypto/blob.rs`: v1 blob (AES-256-GCM, 12-byte
  nonce, 16-byte tag); `MAX_BLOB_LEN = 8 MiB`; `Purpose` is a closed enum
  with `label()`, `max_len()`, `needs_item_id()`, `needs_binding()`;
  `BlobContext.bound_to` is a 32-byte binding (used by `item_details`, CR1).
* `crates/havenkeys-core/src/model.rs`: `ItemDetails` variants `Login`,
  `SecureNote`, `Identity(Box<_>)`, `Card(Box<_>)`. Details are a bad home
  for a new field: older builds silently **drop** an unknown `Login` or
  `SecureNote` field on the next edit. `IdentityFields` and `CardFields` are
  `deny_unknown_fields` (`identity.rs`, `card.rs`), so an older build fails to
  parse them. `open_json` (`vault.rs`) turns that into `Corrupted`, and
  `stage_trash` tombstones a `Corrupted` item. `ItemOverview` has `extra`
  (flattened), so older builds keep an unknown overview field through every
  edit (spec 2026-10-07-item-tags §3.4).
* Server `routes/items.rs` `Change` is `deny_unknown_fields`: an older server
  rejects a write carrying a field it does not know.
* Client `havenkeys-client`: a 409 becomes `ItemChangedElsewhere`; there is no
  automatic merge-and-retry (only restore retries).
* `crates/havenkeys-core/src/trash.rs`: a trashed item keeps its sealed
  details (`trashed_at` on the overview) and is purged after
  `TRASH_RETENTION_MS` (30 days) by writing a tombstone.
* `crates/havenkeys-core/src/export/backup.rs`: an `.hkbackup` is
  `header ‖ one sealed blob` (`Purpose::Backup`, Argon2id key from the backup
  password), capped by `MAX_BACKUP_BYTES`, read whole into memory.
* Server `crates/havenkeys-server/src/limits.rs`: `MAX_BLOB_BYTES = 8 MiB`,
  `MAX_BODY_BYTES = 16 MiB`, `MAX_VAULT_BYTES = 256 MiB`,
  `MAX_PAGE_BYTES = 12 MiB`.
* Server `routes/items.rs` `write`: locks the vault row, checks base
  revisions, bumps `vaults.revision` once, applies changes, then refuses with
  `VaultFull` if `vault_bytes` went over the cap and grew. A delete sets
  `overview`/`details` to NULL.
* Server `main.rs`: a daily sweep task (`erase::sweep_tombstones`).
  `erase.rs`: deleting the account cascades to vaults and items.
* Desktop CSP (`apps/desktop/src-tauri/tauri.conf.json`): `img-src 'self'
  data:`.

## 3. Decisions

| Question | Decision |
|---|---|
| Size cap | **5 MiB** plaintext per file (`MAX_ATTACHMENT_BYTES = 5 * 1024 * 1024`). Owner's choice was 3 or 5 MB; 5 chosen because phone photos of documents are often 3–5 MB. |
| Chunking | **None.** One sealed blob per file. 5 MiB + overhead fits the 8 MiB blob cap, and its base64 (~6.7 MiB) fits the 16 MiB body and 12 MiB page budget. |
| Mutability | **Immutable.** An attachment's bytes never change; "replace" is delete + add with a new id. No conflicts on bytes, and caching is trivial. |
| Which items | All four types (Login, SecureNote, Identity, Card). The Identity is the natural home for RG/CNH. |
| Per item | At most **20** attachments (`MAX_ATTACHMENTS_PER_ITEM`). |
| Per vault | Attachments may use at most **192 MiB** (`MAX_ATTACHMENT_VAULT_BYTES`) of the 256 MiB vault quota, so at least 64 MiB always stays for items (anti-goal 1). |
| Where the list lives | In the item's sealed **overview** (manifest), not details: older builds keep unknown overview fields through `extra`, while they drop or reject unknown details fields (§2). The server never sees names or types. |
| How the server knows what is live | An item write that **changes** the attachment set lists the item's ids in plaintext (`attachmentIds`); the server keeps exactly those and deletes the rest. A write without the field leaves attachments untouched (§4.4). Leaks the number of attachments per item, which upload sizes already reveal. |
| Older servers and clients | No capability endpoint. Older clients never send `attachmentIds` and keep the manifest, so nothing is lost. New clients send it only when attaching or removing; an older server answers the upload with 404 → "Your server doesn't support attachments yet." |
| Sync | Pull is unchanged: the manifest rides the item. Bytes are fetched by id. |
| Device cache | Sealed blobs as received, LRU, size-capped; readable offline; never synced. |

## 4. Design

### 4.1 Model (core)

`ItemOverview` gains, for all item types:

```text
#[serde(default, skip_serializing_if = "Vec::is_empty")]
attachments: Vec<AttachmentMeta>
```

`ItemDetails`, `IdentityFields` and `CardFields` do not change. The
manifest is decrypted at unlock with the other overviews. Every edit path
copies it from the stored overview, the way `trashed_at` and `tags` are
carried. Filenames get the same sensitivity as titles. They are never sent
to the extension (`find_matches` and the other native-messaging replies
build their own response types and must not gain this field).

```text
AttachmentMeta {
  id: Uuid,                 // random v4, server key, never reused
  name: String,             // display/save name, sanitized on input (§4.7)
  mime: String,             // as detected on attach (§4.7); informational
  size: u64,                // plaintext bytes, ≤ MAX_ATTACHMENT_BYTES
  blob_sha256: [u8; 32],    // SHA-256 of the sealed blob as uploaded
  created_at: i64,          // Unix ms
}
```

`AttachmentMeta` follows the codebase's secret-hygiene pattern: redacted
`Debug`, `name` zeroized on drop.

Lists show a paperclip from `attachments.len()`; there is no separate count
field.

### 4.2 Sealing

New `Purpose::Attachment`: label `b"attachment"`, `max_len = MAX_BLOB_LEN`,
`needs_item_id = true`, `needs_binding = true`.

```text
ctx = BlobContext {
  purpose:  Attachment,
  vault_id,                                    // enclosing vault
  item_id:  Some(item_id),                     // owning item
  bound_to: Some(SHA-256("hk-attachment-v1" ‖ attachment_id)),
}
```

* Core refuses plaintext over `MAX_ATTACHMENT_BYTES` **before** sealing.
* On read, the client checks `SHA-256(blob) == meta.blob_sha256` first, then
  opens the blob with the context above. Either failure gives the same error,
  "This attachment is damaged or was changed", and no bytes. The hash pins the
  exact ciphertext the manifest names. The AAD stops a blob from opening under
  another item, vault or attachment id. Together they rule out swaps, replays
  and truncation (anti-goal 4).
* `seal`/`open`/`parse` do not change.

### 4.3 Server storage

Migration `0006_attachments.sql`:

```sql
CREATE TABLE attachments (
  vault_id      UUID NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
  attachment_id UUID NOT NULL,
  item_id       UUID NOT NULL,
  blob          BYTEA NOT NULL,
  committed     BOOLEAN NOT NULL DEFAULT false,
  uploaded_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (vault_id, attachment_id)
);
CREATE INDEX attachments_by_item ON attachments (vault_id, item_id);
```

Limits added to `limits.rs`: `MAX_ATTACHMENT_BLOB_BYTES` (5 MiB plus blob
overhead, rounded up to 5 MiB + 4 KiB), `MAX_ATTACHMENTS_PER_ITEM = 20`,
`MAX_ATTACHMENT_VAULT_BYTES = 192 MiB`, `PENDING_ATTACHMENT_TTL_HOURS = 24`.
The core mirrors the first three.

Endpoints (session-authenticated, vault from the session as today):

* `PUT /v1/attachments/{attachmentId}`, body `{ itemId, blob }` (base64,
  through the existing `b64` decoder; the blob is refused above
  `MAX_ATTACHMENT_BLOB_BYTES`). It is a write, so it is refused for a frozen
  account (`AccountFrozen`). Under the vault row lock:
  * the id must be new for this vault (a re-PUT of the same id, same item and
    same bytes is accepted as a no-op so retries are safe; anything else is
    `Conflict`);
  * refused with a new `AttachmentsFull` error when committed + pending
    attachment bytes would exceed `MAX_ATTACHMENT_VAULT_BYTES`, and
    `VaultFull` when the whole vault would exceed `MAX_VAULT_BYTES`;
  * stored with `committed = false`. It does **not** bump `vaults.revision`,
    because nothing references it yet.
* `GET /v1/attachments/{attachmentId}` returns `{ itemId, blob }` for a
  **committed** row of the session's vault, else `NotFound`. Allowed for
  frozen accounts (reading stays available).

`vault_bytes` (used by item writes) adds `sum(octet_length(attachments.blob))`,
so the 256 MiB ceiling covers everything. Since attachments stop at 192 MiB,
item writes always have at least 64 MiB.

### 4.4 Committing: item writes carry `attachmentIds`

A non-delete `Change` in `POST …/items` gains an **optional**
`attachmentIds: [Uuid]` (at most `MAX_ATTACHMENTS_PER_ITEM`, no duplicates).
The client sends it only when the write adds or removes attachments. When it
is absent, the item's attachment rows are left untouched. When it is present,
inside the existing write transaction and after the conflict checks:

1. Every listed id must exist in `attachments` for this vault **and this
   item** (pending or committed), else the batch is refused
   (`InvalidRequest("unknown attachment")`), and nothing is written.
2. Listed rows become `committed = true`.
3. Rows for this item **not** listed are deleted.

A delete (tombstone) change deletes all of the item's attachment rows.
Trash purge, permanent delete and "empty Trash" all go through tombstones, so
they free the space with no extra code.

Moving an item to the Trash and restoring it re-write the item with the same
manifest, so its attachments stay. Trashed items' attachments still count
toward quota until purged; the Trash UI says so.

**Older clients (anti-goal 2).** A build without this feature keeps the
manifest in the overview's `extra` and never sends `attachmentIds`. Its
edits therefore change neither the manifest nor the stored files. Its
tombstones (permanent delete, Trash purge) delete the files, which is the
correct outcome. Its view of a Card or Identity is unaffected, because
details do not change. Tests: an overview with a manifest survives an edit
made by the previous release's core (fixture), and a change without
`attachmentIds` leaves the item's rows unchanged.

**Older servers.** A server without this feature has no attachment routes and
its `Change` rejects unknown fields. A new client only ever reaches those on
attach or remove, which start with `PUT /v1/attachments/…`. A 404 there maps
to "Your server doesn't support attachments yet — update havenkeys-server."
Nothing else is sent, so every other write keeps working. Server first, then
clients, is still the release order for the hosted server.

**Conflicts and orphans.** The client writes in this order: upload blob(s) →
item write naming them. If that write hits a 409, the user gets today's
`ItemChangedElsewhere` message and attaches again after the refresh. There is
no automatic merge, which keeps the scope small. If the app crashes in between
or the write conflicts, the upload stays pending. The daily sweep deletes
pending rows older than `PENDING_ATTACHMENT_TTL_HOURS`. Pending bytes count
toward the quota, so abandoned uploads cannot grow storage without bound.

### 4.5 Client flows

**Attach** (needs the server, like every write; `require_online`):

1. Rust reads the file (desktop: path chosen in a Rust-side file dialog;
   Android: bytes from the content URI, read by Kotlin with a hard
   `MAX_ATTACHMENT_BYTES + 1` read limit and handed to Rust). Too big → "Files
   can be up to 5 MB".
2. Rust detects type and sanitizes the name (§4.7), seals the blob (§4.2) and
   uploads it.
3. Rust stages the item write with the new `AttachmentMeta` added to the
   overview, through the normal staged-write path (details re-sealed
   unchanged, as the Trash does), sending `attachmentIds`.
4. The sealed blob goes straight into the device cache.

Several files at once are uploaded one by one, then committed in **one** item
write.

**Remove**: item write without that id; the server deletes the row; the
client evicts it from the cache. Removal cannot be undone, so it asks first:
"Remove receipt.pdf? It will be deleted from HavenKeys on all your devices."

**Open / save** (works offline if cached):

* fetch from cache, or `GET` and then cache; verify and decrypt (§4.2);
  plaintext lives in `Zeroizing` buffers;
* **Desktop "Save as…"**: Rust shows the save dialog and writes the file
  itself; the plaintext never crosses into the webview. The dialog warns once
  per session: "A saved copy is not protected by HavenKeys."
* **Android "Save to device"**: system document picker (ACTION_CREATE_DOCUMENT);
  Kotlin writes the bytes Rust returns to the chosen URI and zeroes its copy.
* **Image preview** (desktop and Android, explicit click/tap): only when the
  bytes sniff (§4.7) as PNG, JPEG, WebP or GIF. Desktop: Rust returns the bytes;
  React shows them with a `blob:` URL that is revoked when the preview closes,
  the item changes or the vault locks. The CSP gains `blob:` in `img-src` only.
  Android: decoded in memory, in an activity already under `FLAG_SECURE`. SVG,
  HTML and PDF are never rendered in-app (v1).

**Duplicate item**: attachments are not copied; the UI says so. Copying needs
a re-seal under the new item id and is left for later.

### 4.6 Device cache

Device-local table `attachment_cache (attachment_id PK, item_id, blob,
last_used)` next to `local_blob`, never synced:

* holds the sealed blob exactly as uploaded/downloaded (ciphertext), so the
  lock does not need to clear it, and it stays readable offline;
* capped at 64 MiB desktop / 32 MiB Android, LRU eviction;
* after each sync, entries whose id is in no live or trashed manifest are
  deleted;
* wiped with the rest of local data on sign-out, account deletion and vault
  replacement;
* a cache miss offline → "You're offline — reconnect to open this file."

### 4.7 Hostile input: names and types

* **Name**: Rust keeps only the final path component, strips control
  characters and Unicode bidi overrides (U+202A–202E, U+2066–2069), replaces
  `/ \ : * ? " < > |`, trims dots and spaces at the ends, caps at 120
  characters keeping the extension, and falls back to `attachment` when empty.
  Applied on attach **and** again on save (a manifest from another device is
  hostile input too).
* **Type**: detected from magic bytes in Rust (PNG, JPEG, WebP, GIF, PDF,
  else `application/octet-stream`); the extension or OS-reported type is never
  trusted for rendering decisions.
* Names, types, bytes and sizes are never logged (§40); errors name no file.

### 4.8 Export and backup

* **Bitwarden JSON / CSV**: no attachments (those formats do not carry them).
  The export screen says "N attachments are not included — use a HavenKeys
  backup to keep them."
* **HavenKeys backup** carries attachments, so the export spec's promise that a
  backup carries every item still holds. `.hkbackup` file version 2:
  * same header and Argon2id backup key as v1;
  * then a sequence of length-prefixed sealed records: record 0 is the items
    payload (as today), then one record per attachment
    (`{ itemId, attachmentId, meta, plaintext }`, each ≤ 8 MiB);
  * every record is authenticated and bound to the header, its index and the
    total record count, so reordering, splicing or truncating the file fails
    to open. The exact framing is left to the builder within those rules;
    it must use the existing `Purpose::Backup` and `bound_to` machinery, with
    no new primitive;
  * written and read record by record, not whole-file in memory; the file cap
    becomes `MAX_BACKUP_BYTES` + `MAX_ATTACHMENT_VAULT_BYTES` + framing;
  * v1 files still restore.
* Every attachment's bytes are needed to make a backup. Uncached ones are
  downloaded first (frozen accounts can still read). If any cannot be
  fetched, the export stops and offers two choices: "Try again" or "Export
  without attachments". Attachments are never left out silently.
* **Restore**: each restored item's attachments are re-sealed under the
  restored item's id (and vault) and uploaded before that item's write. If the
  attachment quota runs out, the item is restored without the files that did
  not fit, and the restore report names them.

### 4.9 Surfaces

* **Desktop**: an "Attachments" section on every item's detail view (name,
  size, type icon; Preview for images, Save as…, Remove) and "Add file" in
  edit mode; paperclip + count in the item list. Goes through the impeccable
  check before done.
* **Android**: the same section; "Add file" offers the document picker and the
  system camera. A camera photo over 5 MiB is re-encoded by Android to JPEG
  (longest side 3000 px, quality 85) before it is handed to Rust; a picked
  file over 5 MiB is refused, not shrunk. Impeccable check before done.
* **Extension**: no change in v1. It never receives attachment bytes or
  names; the native-messaging protocol gains no message.
* **iOS / CLI / MCP**: later, over the same Rust API.

## 5. What does NOT change

* Blob format, `seal`/`open`/`parse`, KDF, vault data key — one new
  `Purpose` arm only.
* Single-writer model and the per-vault `revision` cursor; the pull API shape.
* The extension's trust boundary and native-messaging protocol.
* `MAX_VAULT_BYTES` (256 MiB).

## 6. Docs, CLAUDE.md, security review

* **CLAUDE.md amendment** (added when this is approved):

  > Amended on 2026-10-17 by
  > `docs/superpowers/specs/2026-10-13-encrypted-attachments-design.md`:
  > items may hold encrypted file attachments up to 5 MiB each, sealed in Rust
  > under the vault data key and bound to their item. The server stores the
  > ciphertext and learns each item's attachment ids and sizes, never names,
  > types or contents. A file the user saves out of HavenKeys is no longer
  > protected by it. Plaintext exports do not contain attachments; the
  > encrypted backup does.

* **docs/crypto.md**: the `attachment` purpose, its AAD binding, and the
  manifest hash.
* **docs/security-model.md / threat-model.md**: a hostile server can withhold
  or delete attachment blobs (reported as damaged/unavailable, never shown
  wrong), sees per-item attachment counts and sizes, and cannot read, swap,
  replay or move them. A saved or previewed file leaves vault protection
  (preview: in the renderer's memory until closed).
* **docs/security-review.md**: entries for the new purpose and binding,
  `attachmentIds` commit/delete semantics, older-client and older-server behaviour,
  pending-upload sweep, quotas, the ciphertext cache, name/type sanitization,
  `blob:` in the CSP, Save-as written by Rust, and backup v2 framing.
* **docs/server-sync.md / API docs**: the two endpoints, `attachmentIds`,
  `AttachmentsFull`.
* **Plan interplay**: vault key rotation (`2026-10-13-vault-key-rotation`) and
  multiple vaults (`2026-10-13-multiple-vaults`) must re-seal attachments when
  an item changes key or vault; both specs get a note.

## 7. Tests (minimum)

* Core: seal/open round trip; wrong item, vault or attachment id fails; a
  tampered blob or hash mismatch fails with no bytes; plaintext over the cap is
  refused before sealing; manifest carried over by every edit path of all four
  types; trash/restore keeps the manifest; an overview with a manifest survives
  an edit by the previous release's core; no native-messaging reply contains
  attachment data.
* Server: upload caps (blob size, per-item count, attachment quota, vault
  quota); frozen account cannot upload but can download; another vault's
  attachment is `NotFound`; commit rules (unknown id refused, unlisted rows
  deleted, tombstone deletes all, absent `attachmentIds` leaves rows untouched); sweep
  removes only stale pending rows; account erase leaves no attachment rows.
* Backup v2: round trip with attachments; reordered, truncated and spliced
  files refused; v1 still restores; partial-quota restore reports skipped
  files.
* Fuzz (§47): manifest deserialization, name sanitizer, type sniffer, backup
  v2 record reader.

## 8. Build order

1. Core: `Purpose::Attachment`, `AttachmentMeta`, manifest carry-over,
   sanitizer, sniffer, cache table, tests.
2. Server: migration, limits, endpoints, commit semantics,
   sweep, tests.
3. Sync client + core flows (attach, remove, open, save), staged writes.
4. Desktop UI (Tauri commands allowlisted: add, remove, save-as, preview).
5. Backup v2 + export messages.
6. Android (UniFFI bindings, picker, camera, preview, save).
7. Docs, CLAUDE.md amendment, security review.

Code standard for every step: idiomatic Rust (and Kotlin/TypeScript) best
practices, to the point and concise, readable enough for a human to review
quickly: small functions, no speculative abstraction, no dead code. Every UI
surface (desktop, Android) goes through an impeccable review before it is
called done.

## 9. Left to the builder

* Exact framing bytes of backup v2 (within §4.8's rules).
* Tauri command names and UniFFI signatures.
* UI layout details (subject to the impeccable check).
* Whether `GET` returns JSON+base64 (consistent with `items/fetch`) or raw
  `application/octet-stream`. Default: JSON+base64, for consistency.
