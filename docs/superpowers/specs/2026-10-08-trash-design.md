# Trash — design

Date: 2026-10-08. Status: approved in conversation.

## 1. Goal

Deleting an item moves it to a **Trash**, where it stays recoverable for 30
days. After that it is removed for good, as every delete is today. The user
can restore an item, delete it permanently, or empty the Trash at any time.

Desktop and Android ship it together, over the same Rust API. The browser
extension gets nothing new: it never sees the Trash.

## 2. Decisions taken

| Question | Decision |
|---|---|
| Where "trashed" lives | Inside the encrypted overview, as `trashedAt` (approach A). Tombstones that keep their blobs on the server for 30 days (B) rejected: server migration, protocol change, version coordination |
| Server and wire protocol | Unchanged. Trash and restore are ordinary item updates; permanent removal is today's delete (a tombstone) |
| Retention | 30 days from `trashedAt`, purged by the first unlocked, online device after a sync |
| Platforms | Desktop and Android in the same release |
| Delete confirmation | None for moving to Trash (Undo instead); kept for "Delete permanently" and "Empty Trash" |
| Restore | The item comes back exactly as it was: details, passkeys, password history, tags |
| Trashed items in autofill, search, Health, tags, activity, export | Never |
| Encrypted backup | Leaves the Trash out |
| Editing a trashed item | Refused; restore first |
| Identity | Still cannot be deleted, so it never reaches the Trash |

## 3. No CLAUDE.md amendment

Nothing here relaxes a rule. The Trash makes deletion reversible; it adds no
plaintext, no network exposure and no new secret path. Trashed items are
excluded from every fill path (§5).

## 4. Data model

`ItemOverview` (`crates/havenkeys-core/src/model.rs`) gains:

```rust
/// When the item was moved to the Trash, Unix milliseconds; `None` for a
/// live item. `default`: overviews written before the Trash existed.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub trashed_at: Option<i64>,
```

The overview is sealed with the vault's AEAD like any other, so the server
cannot tell a trashed item from a live one. It sees an update with a new
revision. `updated_at` is not changed by trashing or restoring: it keeps
meaning "last edit to the item's content".

Unknown fields in `extra` are kept across trash and restore, as across any
edit (spec 2026-10-07-item-tags §3.4).

## 5. Rust core

### 5.1 Two maps

The unlocked session holds two maps:

* `overviews`: live items, exactly as today.
* `trash`: new, `HashMap<Uuid, ItemOverview>`, items whose `trashed_at` is
  set.

At unlock and on every sync pull, each decrypted overview goes into exactly
one of them, by `trashed_at`. An item never sits in both.

About twenty readers use `session.overviews`: `find_matches`, search,
`list_items`, app fill (Android direct fill), passkey sign-in, Health,
tags, activity and recents, plaintext export, encrypted backup, and the
fill and TOTP paths that look an item up by ID. Because they read only
`overviews`, every one of them leaves trashed items out without being
changed. That is the point of the split: no reader needs a filter that a
future change could forget.

Only the functions in §5.2 read `trash`.

On lock, `trash` is dropped with the session and its strings are zeroized
(by `ItemOverview`'s `Drop`), like `overviews`.

### 5.2 Operations

All of them stage a write, which the client pushes to the server, the
single writer; local state changes only after the server assigns a
revision, as today.

* `stage_trash(id)`: the item must be in `overviews` (else `NotFound`) and
  must not be the identity (else `Denied`). Re-seals the overview with
  `trashed_at = now`; the encrypted details are not touched. On commit the
  item moves from `overviews` to `trash`. This replaces `stage_delete` as
  what the apps' Delete does.
* `stage_restore(id)`: the item must be in `trash` (else `NotFound`).
  Re-seals the overview with `trashed_at = None`. On commit it moves back to
  `overviews`.
* `stage_purge(id)`: the item must be in `trash` (else `NotFound`). Sends
  today's delete (no blobs); the server keeps its tombstone and this device
  removes the row.
* `stage_empty_trash()`: one batch of purges for every item in `trash`.
* `list_trash()`: the trash overviews, newest `trashed_at` first, each with
  the days left before removal (0 to 30).
* `trash_count()`: for the sidebar and Settings row.

`stage_delete` stays as the internal "send a tombstone" step used by purge;
it is no longer reachable from the apps for a live item.

Every update path (`stage_update`, add or remove passkey, TOTP and tag
edits, Health dismissals) looks the item up in `overviews` only, so it
answers `NotFound` for a trashed item.

### 5.3 Expiry

After every successful sync, an unlocked, online device stages purges, in
one batch, for every trash item with `trashed_at` older than 30 days
(`now - trashed_at >= 30 × 86 400 000 ms`). The clock is injectable for
tests. An offline device does it on its next successful sync.

If two devices purge at the same time, the second gets a 409 for the
batch, pulls (which removes the items) and retries with whatever is still
due, which may be nothing.

A device clock far in the future could purge early. The same clock already
stamps `updated_at` and password history; the risk is noted in the
security review rather than defended against.

### 5.4 Sync

`apply_remote_changes` (`crates/havenkeys-core/src/sync.rs`) places each
incoming live change by `trashed_at`: into `trash` when set, into
`overviews` when not, removing it from the other map. A restore done on
another device therefore moves the item back here. A tombstone removes the
item from either map.

The local store needs no schema change: it keeps sealed rows, and the
marker is inside the seal.

### 5.5 Conflicts

Trash and restore are updates with a base revision, so they get the
existing conflict handling:

* Trashed here while edited elsewhere: 409, pull, and the user sees the
  existing "This item changed on another device" message; nothing is lost.
* Restore of an item another device already purged: 409, pull removes it,
  and the user sees "This item was removed from Trash".

### 5.6 Errors

All new messages are generic and name no item, title, username or field
value (CLAUDE.md §39).

## 6. User interface

### 6.1 Desktop

* **Sidebar:** a **Trash** entry after the tools (Health, Generator,
  Settings), with a count only while the Trash is not empty.
* **Delete:** moves the item to the Trash at once, without the inline
  confirm, and shows a toast "Moved to Trash" with **Undo**, which restores
  it. The passkey warning goes, since passkeys come back on restore. Cards
  and logins/notes behave the same.
* **Trash list:** title, username or card summary, and "Deleted N days ago ·
  removed in M days". A line above it says items are removed for good after
  30 days. **Empty Trash** button.
* **Trashed item:** shown overview-only with **Restore** and **Delete
  permanently**. Reveal, copy, edit, TOTP and passkey management are not
  offered; restore first.
* **Delete permanently / Empty Trash:** the inline confirm, naming the item
  (or the number of items), with the passkey warning when any of them holds
  a passkey.
* **Offline:** all Trash actions disabled, as Delete is today.

New Tauri commands: `trash_item`, `restore_item`, `purge_item`,
`empty_trash`, `list_trash`. The existing
`delete_item` command is replaced by `trash_item`; the allowlist and
capability files are updated to match.

### 6.2 Android

* **Where:** a "Trash" row in Settings, with the count. Not an Items
  category: that enum drives filtering across the app.
* **Delete** (item's More menu): moves the item to the Trash without the
  dialog and shows a snackbar "Moved to Trash" with **Undo**.
* **Trash screen:** the same list as the desktop. Tapping an item opens it
  read-only, with Restore and Delete permanently in the More menu; Empty
  Trash in the top bar. Permanent actions confirm in a dialog.
* **Offline:** disabled, as Delete is today.

New UniFFI functions in `havenkeys-mobile` mirror §5.2, each behind
`require_online` for writes.

Both apps follow their `DESIGN.md`; the new screens are reviewed with
impeccable.

## 7. What does not change

* `havenkeys-server`, its schema, routes and quota code.
* The extension and the native-messaging protocol.
* Plaintext export format.

## 8. Known limitations

* **Quota:** trashed items count toward the 256 MiB per vault until they
  are purged. Emptying the Trash frees the space.
* **Expiry needs a device:** if no device unlocks and syncs, items stay
  past 30 days, until one does.
* **Version skew:** a desktop or Android build older than this one keeps
  `trashedAt` in `extra` and treats the item as live: it shows it and can
  fill it. Nothing leaks (it is the user's own item), and an edit made there
  keeps the marker. Desktop and Android ship this together.
* **Backups:** the encrypted backup leaves the Trash out, so a trashed item
  cannot be recovered from a backup taken after it was trashed.

## 9. Testing

**Rust core**

* Trash, restore, purge and empty; each moves the item between maps only
  after the server's answer.
* Expiry with an injected clock: 29 days kept, 30 days purged; the
  concurrent-purge 409 path.
* A trashed item is absent from `find_matches`, search, `list_items`, app
  fill, passkey sign-in, Health, tags, activity and recents, plaintext
  export and the encrypted backup.
* Update, passkey, TOTP and fill calls by ID on a trashed item answer
  `NotFound`.
* The identity cannot be trashed.
* Sync pull places items by `trashed_at`, and a remote restore moves the
  item back.
* Unknown overview fields survive trash and restore.
* Lock clears `trash`.

**Security regression** (CLAUDE.md §46)

* A native-messaging `fill_item`, `get_totp` or passkey assertion for a
  trashed item's ID, from an origin the item matches, is DENIED.
* Android direct fill never offers a trashed login, card or identity.

**Desktop:** Vitest for the Trash list, the trashed-item view and the Undo
toast.

**Android:** ViewModel tests for the Trash screen and the Undo snackbar.

**Server:** no change, so no new tests.

## 10. Documentation

* `docs/security-model.md`: Trash membership is inside the encrypted
  overview; trashed items are excluded from every fill path; quota note.
* `docs/security-review.md`: an entry for the Trash (future-clock early
  purge, version skew).
* `docs/ideas.md`: item 2 points to this spec.

## 11. Revisions from planning

* A trashed item is shown overview-only: title, username or card summary,
  websites, tags, passkey badge, days left. Reveal and copy are not offered;
  restore first. `trashed_item` is dropped, and `get_trashed_item` is not a
  command. This keeps every secret path reading the live map only.
* An item whose encrypted details no longer open is deleted for good by
  Delete, since nothing could restore it; the apps then say "Deleted".
* Android: tapping a trashed item opens a sheet (Restore, Delete
  permanently) rather than a read-only item screen.
* Desktop commands: `trash_item` (returns whether the item is in the Trash),
  `restore_item`, `purge_item`, `empty_trash`, `list_trash`.
* Core names: `stage_trash`, `stage_restore_trashed` (not `stage_restore`,
  which the backup restore already uses), `stage_purge`, `stage_empty_trash`,
  `stage_expired_trash`, `list_trash`, `trash_count`; the constant is
  `TRASH_RETENTION_MS`.
* Android: the Trash is a row in Settings, in a new "Vault" group. Empty
  Trash is under the screen's More menu. Undo shows on whichever screen the
  user lands on (shell, Search or Health).
* pt-BR copy follows each app's existing vocabulary ("excluir", "chave de
  acesso") rather than the wording in the plan.
