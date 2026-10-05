# Import from more password managers — design

Date: 2026-10-05. Status: approved in conversation.

## 1. Goal

Someone moving to HavenKeys from Bitwarden, a browser's built-in password
manager, KeePassXC or LastPass can bring their data over on the desktop, with
the same guarantees the 1Password `.1pux` import already gives: the export is
hostile input, nothing is logged, the report is counts only, the server
stores the items through the normal staged write path, and the user is
offered to delete the plaintext file afterwards.

CLAUDE.md §38 already allows "importing from common password-manager
formats"; no amendment is needed.

## 2. Scope

| Source | Logins | Notes | TOTP | Cards | Other |
|---|---|---|---|---|---|
| Bitwarden JSON (unencrypted) | yes, every URI | yes | `otpauth://` or raw Base32 | yes | identities → converted notes; custom fields → login custom fields (notes for other types); folders ignored; trashed items skipped (`skipped_archived`) |
| Bitwarden CSV | yes | yes (`type=note`) | yes | — (not in the CSV) | `fields` → notes |
| Chrome / Edge / Brave CSV | `name,url,username,password,note` | — | — | — | |
| Firefox CSV | `url,username,password` | — | — | — | `timeCreated`/`timePasswordChanged` kept as timestamps; `httpRealm`, `formActionOrigin` ignored |
| KeePassXC CSV | `Title,Username,Password,URL,Notes,TOTP` | — | yes | — | `Group` ignored |
| LastPass CSV | `url,username,password,totp,extra,name` | rows with `url` = `http://sn` | yes | — | `grouping` ignored; structured secure notes (card, address…) are notes with the `extra` text |

Out of scope: Android, encrypted Bitwarden exports (refused with a message
asking for the unencrypted one — supporting them means reimplementing
Bitwarden's key scheme), KeePass `.kdbx`/XML, folders/groups, attachments,
manual column mapping.

## 3. Core (`crates/havenkeys-core/src/import/`)

* `mod.rs`: `ImportSource` (closed enum: `OnePassword`, `BitwardenJson`,
  `BitwardenCsv`, `Chrome`, `Firefox`, `KeePassXc`, `LastPass`), `Parsed`
  (moved from `onepux.rs`), the shared limits, and
  `parse(source, bytes) -> Result<Parsed>`.
* `common.rs`: helpers the importers share, moved out of `onepux.rs`
  without changing behaviour: `clean_line`, `non_empty`, `Extras` (notes
  lines), `collect_urls` over a list of strings (valid http(s) → URL rules
  with `Domain` matching, the rest kept as "Website:" lines in the notes),
  `set_or_keep`, and a `login`/`note` builder that applies the title,
  username and notes limits.
* `csv.rs`: one defensive reader on the `csv` crate (already in
  `Cargo.lock`). UTF-8 only, a leading BOM is ignored, header names are
  matched case-insensitively and in any order, extra columns are ignored.
  Each source declares its required columns; a header without them refuses
  the file as "not a valid <source> export file". A row the reader cannot
  parse, or whose field count differs from the header, counts in `failed`
  and the rest go on.
* `bitwarden.rs`: the JSON export through `serde_json`, wiped (best effort)
  after conversion like `.1pux`. `"encrypted": true` is refused with its own
  error.
* TOTP: a value that `parse_totp_input` accepts becomes the login's TOTP;
  anything else stays in the notes as "One-time password: …" and counts in
  `fields_to_notes`.
* Bitwarden URI match settings map only where the meaning is the same and
  never looser: host → `Origin`, exact → `Exact`, everything else
  (default/base domain, starts-with, regex, never) → `Domain`.

## 4. Limits

* File: 64 MB for CSV and JSON (the `.1pux` keeps its own 256 MB archive
  limit). Checked on the file's metadata and again on what is read.
* At most `MAX_ITEMS` (50 000) items; more refuses the file.
* Every value still passes `build_item`'s validation in `stage_import`; an
  item that does not counts in `failed`.

## 5. Desktop

* `import_1pux` becomes `import_file(source)`. `source` is the closed enum;
  anything else fails deserialisation. Rust picks the picker's title and
  filter from it; the renderer still never supplies a path. Everything
  after the pick (online check before the picker, `stage_import`,
  `push_batches`, `last_import` for "Delete the export file") is unchanged.
* `ImportSection`: a radio list of the six sources (Bitwarden offers JSON
  and CSV), one or two lines on how to export from the chosen one, then
  "Choose file…". The plaintext warning and the delete flow stay.
* New error strings get pt-BR translations.

## 6. Security

Unchanged posture: the file is read into `Zeroizing` memory, values are
wrapped in `SecretString` as soon as they are taken, the JSON tree is
wiped, `Debug` on imported items is redacted, errors never echo content,
counts are the only output. CSV formula prefixes (`=`, `+`, `@`) are
harmless here: we only read and store values literally.

## 7. Testing

* Per source: synthetic exports written from the public formats (no real
  data) covering a full login, several URLs, a non-http URL, `otpauth://`,
  raw Base32 and invalid TOTP, notes, cards and identities (Bitwarden),
  trashed items (Bitwarden), `http://sn` (LastPass), BOM, reordered
  columns, a missing required column, a broken row.
* Refusals: one source's file imported as another, the encrypted Bitwarden
  export, an empty file.
* The existing `onepux` tests pass unchanged after the move to `common.rs`.
* Deterministic fuzz: mutated CSV and JSON for every source never panic.
* Desktop: `ImportSection` calls `importFile(source)` with the chosen
  source; the Tauri crate builds and its tests pass.
