# Item tags — design

Date: 2026-10-07. Status: approved in conversation; revised after
implementation (see "Revisions from planning and implementation" below).

> **Revision 2026-10-07 (owner request): tags keep their case, brass pills.**
>
> * Tags keep the case the user gave them ("Dev Team"); whitespace is still
>   trimmed and collapsed. Duplicates are found without case.
> * The vault keeps one spelling per tag: when an item is saved, Rust maps
>   each of its tags to the spelling other items already use (the item
>   itself left out, so the only item with a tag can change its case).
>   Where older data holds several spellings of one tag, the one most items
>   carry is used (ties: the smallest by code point), in Rust and in both UIs.
> * Concurrent creation of "Work" and "work" on two devices can leave two
>   spellings until either item is saved again; the UIs merge spellings
>   without case (sidebar / Items tab counts, the tag filter, suggestions).
> * Tag pills use the brass pill (brass wash, brass ink) on desktop and
>   Android; vault-health chips stay outlined, so the two differ by colour.
>   The extension is unchanged. In light theme the desktop's tag pills use a
>   darker brass ink (`--tag-ink` #6f5728) to read on the wash; Android's
>   `brass-ink-light` (#7d632f) already reads 4.66:1 or more there.
> * The tag editor offers the vault's most-used tags (those the item lacks,
>   up to 8 on the desktop and 5 on Android) when its empty field gains
>   focus; typing narrows them.
>
> §2 "Case", §3.2 and §4 below are updated to match.

> **Revisions from planning and implementation** (plan
> `docs/superpowers/plans/2026-10-07-item-tags.md`, "Deviations from the
> spec"):
>
> 1. `ItemInput.tags` is `Option<Vec<String>>`: `None` keeps the stored tags,
>    `Some` replaces them (the `sections` pattern). Save paths that do not
>    know about tags (extension save-login, Android direct saves, imports
>    matching an existing login) send `None` and keep them. On Android only
>    the editor draft sends tags.
> 2. No Rust `list_tags` and no tag argument on `list_items`. Desktop and
>    Android already hold every overview in memory; each UI computes tag
>    counts and the tag filter from those overviews, as it does category
>    counts.
> 3. The extension popup has no search box, so "popup search matches tags"
>    is dropped.
>
> Smaller decisions taken while building:
>
> * Bitwarden export folder ids are a UUID v8 built from the first 16 bytes
>   of SHA-256(tag): stable per tag, no new dependency.
> * Desktop tag editor: suggestions start with no active option, so Enter
>   adds the typed text unless an arrow key picked a suggestion; the
>   suggestion list renders in the row's flow (the group clips anything
>   floating); typed text is committed when the field loses focus.
> * Extension: tags show on login rows only (not on OTP rows).
> * Android: the Add tag field also commits on blur and on Save, and a Plus
>   button adds; the kit's `InsetGroupScope.row` takes an optional key.
>
> Fixed after the final branch review:
>
> * Android: a tag's name is no longer a navigation argument (it reached the
>   Activity's saved-state Bundle, against security model §22.12). The tag
>   list's route is a fixed `items/tag`; the name stays in memory in a
>   ViewModel on the shell's entry, keyed by the back stack entry's id, and
>   goes with the shell on lock. A tag list with no name there (process
>   death) backs out to Items.
> * Android: a tag list whose tag no item carries any more (untagged,
>   deleted, or a sync) goes back to Items, as the desktop falls back to All
>   items.
> * Android: the Items tab's tags and the editor's suggestions sort with the
>   titles' `Collator`, so accented tags sort as on the desktop
>   (`localeCompare`).
> * Bridge: a `find_matches` answer over `MAX_RESPONSE_BYTES` (50 logins with
>   the longest titles, usernames and 20 tags each) drops tags, last match
>   first, until it fits, instead of failing the frame and the connection. No
>   protocol change.
> * Desktop and Android: Save with a tag Rust would refuse in the tag field
>   does not save; the field keeps the text and says why. A refused part of
>   a comma-separated list stays in the field instead of being dropped. On
>   Android, tag text typed but not added makes Back ask before discarding.
> * §3.4 now says that restoring a backup does not keep `extra`.

## 1. Goal

Let the user separate items by context — work, development, staging,
production, personal — with free-form tags they create on any item and reuse
on others. Two payoffs:

* **Organise the vault**: filter the desktop and Android lists by a tag;
  search matches tags.
* **Tell logins apart while filling**: when one site has several logins
  (dev, staging, prod admin), the autofill menu and popup show each one's
  tags, very small, on the username line.

Tags never change *which* logins autofill offers. There is no "active tag"
mode that hides items.

## 2. Decisions taken

| Question | Decision |
|---|---|
| What tags do in autofill | Labels only (option B): shown on the suggestion; no grouping, no scoping |
| Extension grouping setting | None. Matches per site are usually 1–3; group headers would cost more than they separate |
| Which items | All item types (login, secure note, card, identity) |
| Tag vocabulary | Free-form; no separate registry. The vault's tag set is the union of item tags |
| Case | Kept as typed; compared without case. The vault keeps one spelling per tag (revision 2026-10-07) |
| Colours | None (both DESIGN.md files forbid a second accent) |
| Where stored | Inside the sealed `ItemOverview`; no server or SQLite change. `ItemInput.tags`: `None` keeps the stored tags, `Some` replaces them |
| Older clients | An edit from an older version drops the item's tags; documented, not worked around. From this version, unknown overview fields survive edits |
| Import | Bitwarden folders, CSV `folder`/`grouping`, 1Password tags → tags |
| Export | Bitwarden JSON: first tag → folder. Backup: automatic. CSV: unchanged |
| Protocol compatibility | `Match` gains `tags`; desktop and extension ship together in one version bump |

No CLAUDE.md amendment is needed: tags are non-secret metadata sealed like
the title and username (§36), and autofill authorization is unchanged.

## 3. Data model and rules (Rust core)

### 3.1 Storage

* `ItemOverview` (`crates/havenkeys-core/src/model.rs`) gains
  `#[serde(default)] tags: Vec<String>`. It is sealed with the rest of the
  overview, so the server and the local `items` table see nothing new.
* `ItemInput` gains `#[serde(default)] tags: Option<Vec<String>>`;
  `build_item` normalises them and writes them into the overview. On update,
  `Some` replaces the stored tags and `None` keeps them, so save paths that
  do not know about tags cannot erase them.
* Items written before this version read as `tags: []`. No migration.

### 3.2 Normalisation (`crates/havenkeys-core/src/tags.rs`)

One function every write path goes through (`build_item`, import):

1. Trim; collapse internal whitespace runs to one space. Case is kept.
2. Reject: empty, more than 32 characters (counted in `char`s), any control
   character, or a comma (the editor uses comma as a separator).
3. Deduplicate without case (the key is Unicode-aware `to_lowercase`; the
   first spelling given wins) and sort by (key, spelling).
4. Reject more than 20 tags on one item.
5. On every staged write, each tag takes the spelling other items already
   use for its key (the item being written is left out); where they use
   several (older data), the spelling most items carry wins, ties going to
   the smallest by code point. Imports and restores carry the spellings of
   their earlier writes.

Two devices that each create a spelling of a new tag before syncing leave
two spellings until either item is saved again; desktop and Android merge
them without case wherever they list or filter tags.

Errors are fixed strings ("Tag is too long.", "Too many tags.", "Tag contains
a character that is not allowed.") and never echo the input.

### 3.3 Reads

* Rust exposes no tag list and no tag filter. Desktop and Android already
  hold every item overview in memory while unlocked; each UI computes the
  tags in use (sorted by name, with counts) and the tag filter (an exact
  match on the normalised tag, combined with the search query) from those
  overviews, as it does for categories. Nothing is cached on disk.
* `search` also matches a query against an item's tags (same substring rule
  as titles).

### 3.4 Keeping unknown overview fields

`ItemOverview` gets a `#[serde(flatten)] extra: serde_json::Map` catch-all.
`stage_update` copies `extra` from the stored overview into the rebuilt one,
so a field added by a future version survives an edit made on this one. This
does not rescue tags from versions released before it; that gap is accepted
(section 7).

Restoring an encrypted backup does not keep `extra` either: a backup is
hostile input, so restore rebuilds every item through `build_item` from what
`ItemInput` carries (tags included), and the rebuilt overview has no unknown
fields. A field added by a future version is lost when its backup is
restored by this one.

## 4. UI

Visual language for every surface: a tag is a **brass pill** (brass wash,
brass ink, with the tag glyph on Android) in the label face, never mono
(revision 2026-10-07; it was an outlined muted pill). Vault-health chips stay
outlined, so tags and health differ by colour. In list rows (desktop
sidebar, Android Items tab and editor rows) the tag glyph is brass and the
name keeps the normal ink. The extension keeps its text-only tags. No colours
per tag. Tags are shown in their stored case; lists and filters compare
without case.

### 4.1 Desktop — editor

* One **Tags** row in the item's group (Row Is the Field rule: no box around
  the input). Existing tags show as pills with a small × button, followed by
  an inline "Add tag" input.
* Focusing the empty input offers the vault's most-used tags the item lacks
  (up to 8, most used first, then A–Z), counted over the other items.
* Typing opens a suggestion popover (floating-menu style: raised, 11px
  radius, lift shadow) listing matching vault tags with their counts, plus a
  last row "Create "<text>"" when the text is not an existing tag.
* Enter or comma adds; Backspace in an empty input removes the last tag;
  arrow keys move through suggestions; Escape closes the popover.
* The input stops accepting characters at 32. At 20 tags the input is
  replaced by a muted line ("20 tags is the limit.").
* Validation errors from Rust show inline under the row, never in a toast.

### 4.2 Desktop — sidebar, list and detail

* The sidebar gains a **Tags** section under the categories: standard 32px
  nav rows with a tag glyph, the tag name and a tabular count, sorted A–Z.
  The section is absent when no item has a tag.
* Selecting a tag works like selecting a category: it becomes the single
  current row (brass wash), the list head's serif title becomes the tag name,
  and the search field narrows within it.
* List rows are unchanged (no chips), keeping the 300px list calm.
* The detail pane shows a **Tags** row of pills when the item has any;
  clicking a pill selects that tag in the sidebar.

### 4.3 Android

* The Items tab gains a **Tags** group under the categories, one row per tag
  with its count; tapping opens a tag list laid out exactly like a category
  list (28sp serif large title, the same back chevron).
* The editor gains a **Tags** group modelled on the websites group: one row
  per tag with a 48dp remove button (pills are markers and have no target),
  then an **Add tag** row with an inline field. Focusing the empty field
  offers up to 5 of the other items' tags the item lacks, most used first,
  then A–Z, as rows below it; typing narrows them (without case). Tapping
  one adds it and keeps the field focused.
* Item detail shows the tags as a row of brass pills with the tag glyph.
* Tags are never cut with an ellipsis (Android rule); a long tag
  wraps.

### 4.4 Extension — in-page menu and popup

* Tags go on the **title line**, after the title, as the smallest text on
  the row in brass ink (10.5px, 600): `Acme  staging`. The username line
  stays plain. (Revised 2026-10-10 at the owner's request; they were faint
  muted text after the username.)
* At most 2 tags, then "+N". Text, not pills: Fill is the row's one brass
  control.
* Frame geometry does not change (Fixed Frame Rule: rows stay 46px; no
  change to `content/frames.ts`). On overflow the tag text is what gets
  truncated, never the title.
* The popup's site rows do the same. (The popup has no search box.)
* Tags show on login rows only, not on one-time-code rows.
* The extension only receives the tags of logins already matched to the
  current origin. It never receives the vault's tag list.

### 4.5 States

* No tags anywhere: no Tags section on desktop or Android; no tag text in the
  extension.
* A tag disappears from the sidebar, the Items tab and suggestions once no
  item uses it.
* Offline (read-only): the editor is already gated, so tags cannot change;
  filtering and search keep working.
* A desktop tag filter whose tag disappears (last item untagged or deleted)
  falls back to All items.

## 5. Interfaces

* **Tauri:** no new command. Item DTOs carry `tags`; the item input DTO
  accepts an optional `tags` (absent keeps the stored ones).
* **UniFFI (`havenkeys-mobile`):** `ItemSummary` and the editor input record
  gain `tags`. Regenerate the Kotlin bindings.
* **Native messaging:** `Match` (`crates/havenkeys-protocol/src/message.rs`)
  gains `tags: Vec<String>`; the response validator caps it at 20 entries of
  32 characters. The TS validator in `packages/protocol/src/index.ts` adds
  `tags` to its exact key list with the same caps. Because both sides check
  exact keys, the desktop and extension must be released in the same version
  bump; the release notes say so. `PROTOCOL_VERSION` stays 1, as with earlier
  `Match` additions.

## 6. Import and export

* **Import:**
  * Bitwarden JSON: each item's folder name becomes a tag; nested folders
    (`Work/Staging`) become one tag with the `/` kept (`work/staging`).
  * CSV: a `folder` or `grouping` column (Bitwarden, LastPass) becomes a tag.
  * 1Password `.1pux`: the item's tags become tags, and stop being appended
    to the notes.
  * A value that fails normalisation (too long, comma, over 20) is not
    dropped: it is appended to the item's notes as `Tags: …`, as today.
* **Export:**
  * Bitwarden JSON: `folders` lists each distinct first tag; an item's
    `folderId` is its first tag (alphabetical, so deterministic). Other tags
    are not exported there (Bitwarden allows one folder).
  * Encrypted backup: `BackupItem` serialises the whole overview, so tags are
    included and restored without change.
  * CSV: unchanged (keeps the Chrome column set other managers import).

## 7. Compatibility and limitations

* No server change, no SQLite change, no vault format version bump.
* An item edited by a HavenKeys version from before this one loses its tags,
  because that version rebuilds the overview without the field. The owner
  updates their own devices; this is documented in the release notes and
  `docs/architecture.md`, not worked around.
* Desktop/extension version skew breaks suggestions in either direction
  until both are updated (exact-key validation on both sides).
* Tags are encrypted at rest and on the server, like titles. The extension
  sees the tags of matched logins only, as it already sees their titles and
  usernames.

## 8. Testing

* **Rust core:** normalisation (case, whitespace, duplicates, 32-char and
  20-tag limits, control characters, comma, Unicode lowercase); errors never
  contain the input; `None` keeps and `Some` replaces stored tags; search
  matching tags; `extra` overview
  fields survive `stage_update`; items without the field read as `[]`.
* **Import/export:** each importer's folder/tag mapping, nested folders,
  invalid values falling back to notes; Bitwarden export's folders and
  `folderId`; backup round trip keeps tags; CSV output unchanged.
* **Protocol:** Rust and TS `Match` accept tags within limits and reject
  oversized lists or tags; extend the protocol fuzz targets with the field.
* **Desktop (TS):** the tag editor (add by Enter/comma, remove, Backspace,
  suggestions, 32/20 limits), tag counts and the tag filter computed from the
  overviews, sidebar section appears/disappears, selecting a tag filters the
  list, detail pill selects the tag.
* **Extension:** the menu and popup render tags on the username line, cap at
  2 + "+N", never truncate the username.
* **Android:** ViewModel tag list and filter; editor add/remove; screenshot
  tests of the editor, the Tags group and a tag list, light and dark, 360dp
  and 411dp.
* **UI review:** `/impeccable` critique of the desktop and Android screens
  against each DESIGN.md, as in earlier stages.

## 9. Documentation

* `docs/native-messaging.md`: the `Match.tags` field and its limits.
* `docs/security-model.md`: tags listed with the other sealed overview
  metadata (not plaintext anywhere).
* `docs/architecture.md`: the older-client limitation from section 7.
* `apps/desktop/DESIGN.md` and `apps/android/DESIGN.md`: the tag pill, the
  Tags sidebar/Items section and the tag editor row.

## 10. Out of scope

* Renaming or deleting a tag across all items (a later Settings action).
* Tag colours or icons.
* Nested tag hierarchy beyond the literal `/` in a name.
* Scoping autofill or the vault to an "active" tag.
* Grouping suggestions by tag in the extension.
* Tags in the CSV export.
