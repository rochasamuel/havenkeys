# Security Fixes CR1, SV-2, EX-01, BR-1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the four Medium findings of the last security scan: an item's two blobs can no longer be spliced across versions (CR1/SV-1); pulls and fetches are cut by bytes so a large vault never wedges sync, and the server no longer loads unbounded rows into memory (SV-2, the paging half of SV-3); popup Fill refuses opaque-origin (sandboxed) documents (EX-01); and on Windows the native host refuses a bridge pipe served by another user (BR-1/P10).

**Architecture:** CR1 binds an item's details blob to its overview blob by putting `SHA-256(overview blob)` in the details blob's AEAD associated data; every open of a details blob needs the overview it was written with. SV-2 sizes rows before reading them (`octet_length`), keeps whole revisions up to a 12 MiB budget, then reads only those blobs; fetch answers as many items as fit and names the rest `unanswered`, which the client asks for again. EX-01 compares `self.origin` (opaque `"null"` in a sandboxed document) with `location.origin` before filling. BR-1 reads the pipe peer's process ID, opens its token and compares its user SID with ours, on both ends of the pipe.

**Tech Stack:** Rust (`havenkeys-core` AES-256-GCM blobs, `sha2`; `havenkeys-server` axum + tokio-postgres; `havenkeys-sync-client`; `havenkeys-client`; `havenkeys-protocol` with `interprocess` 2.4 and `windows-sys` 0.61), TypeScript extension (vitest + jsdom).

**Spec:** `docs/security-review.md` — findings CR1/SV-1, SV-2 (with the paging part of SV-3), EX-01, BR-1 (with P10), each with its "Suggested fix". Project rules: `CLAUDE.md` (§6 encrypted format, §19 content-script security, §30–31 native messaging, §45–46 tests).

## Global Constraints

- **Vault format change, no migration:** after CR1, details blobs written before this change no longer open. The only vault in existence is the owner's, and format changes skip migrations (project decision): the owner resets the vault and the server account after updating. Say so in `docs/crypto.md` and in the final report; do not write migration code.
- Do not invent cryptography: CR1 uses the existing AES-256-GCM blob and SHA-256 (`sha2`), only changing what goes into the associated data.
- The server stays blind: it never parses blobs; sizes come from `octet_length` in SQL.
- One write batch is already bounded by `MAX_BODY_BYTES` (16 MiB), so a page holding one whole revision always fits the client's `MAX_RESPONSE_BYTES` (17 MiB). Pages still end only at a revision boundary.
- `MAX_PAGE_BYTES = 12 * 1024 * 1024`, counted as base64 length of every blob plus `ROW_JSON_BYTES = 256` per row.
- A fetch answer must never let the client mistake an unanswered item for a deleted one.
- No secret in any log, error or `Debug` output (CLAUDE.md §39–40). Errors stay fixed sentences.
- Windows code must compile: `cargo check --target x86_64-pc-windows-gnu` and `cargo clippy --target x86_64-pc-windows-gnu` on the touched crates (the target is installed). It cannot run here; the manual Windows check is recorded in `docs/security-review.md`.
- Extension: no `innerHTML`, no new permissions; tests in vitest (`npm test` in `apps/extension`), `npm run typecheck` and `npm run lint` if those scripts exist.
- Code style: simple, readable, clear names; comments only where the reason is not obvious (mostly security). No narration.
- Commits carry no Co-Authored-By trailer.

## Review Focus

1. **A vault holding items written before CR1** — expected: those items count as damaged/unreadable, never panic, never open with the old context; the owner resets. Pinned by Task 1's `an_unbound_details_blob_does_not_open`.
2. **One single item bigger than the page budget (an 8 MiB note)** — expected: a page with just that revision still goes out and the cursor advances; the loop never stalls. Pinned by Task 2's `page_len_always_takes_the_first_revision` and `a_vault_of_big_items_pulls_in_pages_under_the_cap`.
3. **The fetch window races a write (an item rewritten between the size query and the blob query)** — expected: it is either answered with its current row or arrives on a later pull; nothing is reported deleted. Pinned by Task 3's `unanswered_items_are_asked_again_and_never_deleted`.
4. **A sandboxed frame inside a normal page, and a normal page (no sandbox)** — expected: the sandboxed one gets nothing, the normal one still fills. Pinned by Task 4's `refuses_a_popup_fill_into_an_opaque_origin_document` next to the existing fill test.
5. **The desktop app running elevated (administrator) while the native host is not** — expected: same user SID, the connection is accepted. Pinned by the comparison being on the token *user* SID (not owner or integrity level) in Task 5 and the manual Windows check.

---

## File Structure

```text
crates/havenkeys-core/
  src/crypto/blob.rs        BlobContext.bound_to; BlobContext::item_details(vault, item, overview_blob)
  src/store.rs              + item_blobs(id) -> (overview, details)
  src/vault.rs              stage() and load_details() use item_details(...)
  src/sync.rs               check_item_bytes uses item_details(...); splice test
  tests/fuzz.rs             details context built with item_details
crates/havenkeys-server/
  src/limits.rs             + MAX_PAGE_BYTES
  src/routes/sync.rs        size query, page_len, blob query; b64_len, ROW_JSON_BYTES
  src/routes/items.rs       fetch: byte budget, `unanswered`
  tests/sync.rs             big-item paging and fetch tests
crates/havenkeys-sync-client/
  src/wire.rs               FetchDto.unanswered
  src/client.rs             fetch_items -> Fetched { changes, unanswered }
  src/transport.rs          comment on MAX_RESPONSE_BYTES
  tests/hostile.rs          unanswered validation
crates/havenkeys-client/
  src/sync.rs               re-ask unanswered items
crates/havenkeys-protocol/
  Cargo.toml                windows-sys (Windows only)
  src/endpoint.rs           Windows connect checks the pipe server's user
  src/win_identity.rs       NEW (Windows): process_is_current_user(pid)
crates/havenkeys-bridge/
  src/server.rs             Windows peer_is_same_user uses process_is_current_user
apps/extension/src/
  content/index.ts          handleFill refuses opaque-origin documents
  content/content.test.ts   test
  webauthn/bridge.ts        bridge falls back in opaque-origin documents
  webauthn/bridge.test.ts   test
docs/crypto.md, docs/server-sync.md, docs/native-messaging.md, docs/threat-model.md,
docs/security-review.md, docs/security-model.md (where they describe these behaviours)
```

---

### Task 1: Bind an item's details blob to its overview blob (CR1/SV-1)

**Files:**
- Modify: `crates/havenkeys-core/src/crypto/blob.rs`
- Modify: `crates/havenkeys-core/src/store.rs` (next to `item_details`, ~line 314)
- Modify: `crates/havenkeys-core/src/vault.rs` (`load_details` ~1176, `stage` ~1780)
- Modify: `crates/havenkeys-core/src/sync.rs` (`check_item_bytes` ~448, tests)
- Modify: `crates/havenkeys-core/tests/fuzz.rs:78`
- Modify: `docs/crypto.md`, `docs/security-review.md`, `docs/threat-model.md`

**Interfaces:**
- Produces:
  - `pub struct BlobContext { pub purpose: Purpose, pub vault_id: Uuid, pub item_id: Option<Uuid>, pub bound_to: Option<[u8; 32]> }`
  - `pub fn BlobContext::item_details(vault_id: Uuid, item_id: Uuid, overview_blob: &[u8]) -> Self`
  - `pub fn Store::item_blobs(&self, id: &Uuid) -> Result<Option<(Vec<u8>, Vec<u8>)>>`
  - `BlobContext::item(Purpose::ItemDetails, …)` now fails to seal or open (`InvalidInput("blob context")`).

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module of `crates/havenkeys-core/src/crypto/blob.rs` (it already has a key helper and `seal`/`open` round trips — reuse whatever key constructor the existing tests use; below it is called `key()`):

```rust
    #[test]
    fn details_open_only_with_the_overview_they_were_written_with() {
        let (vault, item) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let k = key();
        let ctx = BlobContext::item_details(vault, item, b"overview-v2");
        let blob = seal(&k, &ctx, b"details-v2").unwrap();
        assert_eq!(open(&k, &ctx, &blob).unwrap().as_slice(), b"details-v2");
        let other = BlobContext::item_details(vault, item, b"overview-v1");
        assert!(open(&k, &other, &blob).is_err());
    }

    #[test]
    fn an_unbound_details_blob_does_not_open() {
        let (vault, item) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let k = key();
        let unbound = BlobContext::item(Purpose::ItemDetails, vault, item);
        assert!(seal(&k, &unbound, b"x").is_err(), "details must be bound to an overview");
        // A blob sealed the old way (no binding in the AAD) does not open
        // under the new context.
        let old_ctx = BlobContext { bound_to: None, ..BlobContext::item(Purpose::ItemOverview, vault, item) };
        let old = seal(&k, &old_ctx, b"x").unwrap();
        assert!(open(&k, &BlobContext::item_details(vault, item, b"ov"), &old).is_err());
    }

    #[test]
    fn only_details_carry_a_binding() {
        let (vault, item) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let bound_overview = BlobContext {
            bound_to: Some([0; 32]),
            ..BlobContext::item(Purpose::ItemOverview, vault, item)
        };
        assert!(seal(&key(), &bound_overview, b"x").is_err());
    }
```

(If the module's helper functions are named differently — e.g. `seal_bytes`/`open_bytes`, or the key comes from `Key256::random()` — use those; the assertions stay as written.)

Append to the `tests` module of `crates/havenkeys-core/src/sync.rs` (it has `activated_vault()`, `login()`, `NOW`, and imports `ItemInput`, `MatchType`, `SecretUpdate`, `UrlRule`):

```rust
    #[test]
    fn a_spliced_item_is_refused() {
        let mut v = activated_vault();
        let first = v.stage_create(login("GitHub"), NOW).unwrap();
        let id = first.item_id;
        let (ov1, det1) = (first.overview.clone().unwrap(), first.details.clone().unwrap());
        v.commit_write(first, 1).unwrap();

        let mut moved = login("GitHub");
        moved.urls = vec![UrlRule {
            url: "example.com".into(),
            match_type: MatchType::Domain,
        }];
        moved.password = SecretUpdate::Set(SecretString::from("pw2"));
        let second = v.stage_update(&id, moved, NOW).unwrap();
        let (ov2, det2) = (second.overview.clone().unwrap(), second.details.clone().unwrap());
        v.commit_write(second, 2).unwrap();

        let change = |ov: &[u8], det: &[u8], revision: i64| RemoteChange {
            item_id: id,
            revision,
            overview: Some(ov.to_vec()),
            details: Some(det.to_vec()),
            deleted: false,
        };
        // Old website rules with the new password, and the reverse.
        let a = v.apply_remote_changes(3, vec![change(&ov1, &det2, 3)], NOW).unwrap();
        assert_eq!(a.skipped_items, 1);
        let b = v.apply_remote_changes(4, vec![change(&ov2, &det1, 4)], NOW).unwrap();
        assert_eq!(b.skipped_items, 1);
        // A pair written together still applies.
        let c = v.apply_remote_changes(5, vec![change(&ov2, &det2, 5)], NOW).unwrap();
        assert_eq!(c.skipped_items, 0);
        assert_eq!(v.get_item(&id).unwrap().urls[0].url, "example.com");
    }
```

(`RemoteChange`, `stage_update` and `SecretString` are in scope of `sync.rs`' tests via `use super::*` and the crate's existing imports; add any missing `use` the compiler names.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p havenkeys-core --lib blob:: sync::tests::a_spliced_item_is_refused`
Expected: compile errors (`item_details`, `bound_to` not found).

- [ ] **Step 3: Implement — `blob.rs`**

Add `bound_to` and the constructor, and extend the AAD:

```rust
/// Context that every blob is cryptographically bound to.
#[derive(Clone, Copy, Debug)]
pub struct BlobContext {
    pub purpose: Purpose,
    pub vault_id: Uuid,
    pub item_id: Option<Uuid>,
    /// Details only: SHA-256 of the overview blob written with them, so the
    /// two halves of one item version cannot be paired with another
    /// version's (security-review CR1).
    pub bound_to: Option<[u8; 32]>,
}

impl BlobContext {
    pub fn vault(purpose: Purpose, vault_id: Uuid) -> Self {
        Self {
            purpose,
            vault_id,
            item_id: None,
            bound_to: None,
        }
    }

    pub fn item(purpose: Purpose, vault_id: Uuid, item_id: Uuid) -> Self {
        Self {
            purpose,
            vault_id,
            item_id: Some(item_id),
            bound_to: None,
        }
    }

    /// An item's details, bound to the overview blob they were written with.
    pub fn item_details(vault_id: Uuid, item_id: Uuid, overview_blob: &[u8]) -> Self {
        use sha2::{Digest, Sha256};
        Self {
            bound_to: Some(Sha256::digest(overview_blob).into()),
            ..Self::item(Purpose::ItemDetails, vault_id, item_id)
        }
    }

    fn aad(&self, version: u8, algorithm: u8) -> Result<Vec<u8>> {
        if self.purpose.needs_item_id() != self.item_id.is_some()
            || (self.purpose == Purpose::ItemDetails) != self.bound_to.is_some()
        {
            return Err(Error::InvalidInput("blob context"));
        }
        let mut aad = Vec::with_capacity(96);
        aad.extend_from_slice(AAD_PREFIX);
        aad.push(version);
        aad.push(algorithm);
        aad.extend_from_slice(self.purpose.label());
        aad.push(0);
        aad.extend_from_slice(self.vault_id.as_bytes());
        if let Some(item_id) = self.item_id {
            aad.extend_from_slice(item_id.as_bytes());
        }
        if let Some(overview) = self.bound_to {
            aad.extend_from_slice(b"overview\0");
            aad.extend_from_slice(&overview);
        }
        Ok(aad)
    }
}
```

`Sha256::digest(..).into()` converts to `[u8; 32]` with `sha2` 0.11; if the compiler refuses the `into()`, use `let d = Sha256::digest(overview_blob); let mut h = [0u8; 32]; h.copy_from_slice(&d);`. Update every literal `BlobContext { … }` in this file's tests with `bound_to: None` (or `Some(..)` where a details context is meant), and update the module doc comment ("Associated data binds each blob to its purpose, vault and item, a details blob to its overview blob, plus the header bytes above.").

- [ ] **Step 4: Implement — store, vault, sync, fuzz**

`crates/havenkeys-core/src/store.rs`, beside `item_details`:

```rust
    /// Both blobs of one item, as written together.
    pub fn item_blobs(&self, id: &Uuid) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT overview, details FROM items WHERE id = ?1",
                params![id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }
```

Remove `item_details` if nothing else calls it (`grep -rn "item_details(" crates`); otherwise keep it.

`crates/havenkeys-core/src/vault.rs` `load_details`:

```rust
    pub(crate) fn load_details(&self, id: &Uuid) -> Result<ItemDetails> {
        let session = self.session()?;
        let overview = session.overviews.get(id).ok_or(Error::NotFound)?;
        let (ov_blob, data) = self.store.item_blobs(id)?.ok_or(Error::NotFound)?;
        let ctx = BlobContext::item_details(session.vault_id, *id, &ov_blob);
        let details: ItemDetails =
            open_json(&session.data_key, &ctx, &data).map_err(|e| match e {
                Error::Corrupted => Error::Corrupted,
                _ => Error::Decryption,
            })?;
        if details.item_type() != overview.item_type {
            return Err(Error::Corrupted);
        }
        Ok(details)
    }
```

`stage`:

```rust
        let det_blob = match details {
            Some(d) => Some(seal_json(
                &session.data_key,
                &BlobContext::item_details(session.vault_id, id, &ov_blob),
                d,
            )?),
            None => None,
        };
```

`crates/havenkeys-core/src/sync.rs` `check_item_bytes`:

```rust
        let details: ItemDetails = open_json(
            data_key,
            &BlobContext::item_details(vault_id, id, &ov_blob),
            &det_blob,
        )
        .ok()?;
```

`crates/havenkeys-core/tests/fuzz.rs:78`: replace `BlobContext::item(Purpose::ItemDetails, Uuid::from_u128(1), Uuid::from_u128(2))` with `BlobContext::item_details(Uuid::from_u128(1), Uuid::from_u128(2), b"overview")` (drop the now-unused `Purpose` import if the compiler says so).

Run `cargo build --workspace` and fix any other `BlobContext` literal or `Purpose::ItemDetails` use it reports (the desktop `src-tauri` has none today).

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS. Every existing test that creates items through `stage_*` keeps working because sealing and opening go through the same binding.

- [ ] **Step 6: Documentation**

- `docs/crypto.md`, where the blob AAD is described: the details AAD now ends with `"overview\0" ‖ SHA-256(overview blob)`; why (CR1: an old overview could otherwise be paired with newer details, sending the current password to a website the item no longer names); and that details written before this change do not open — the vault is reset, no migration (format changes skip migrations while only the owner's vault exists).
- `docs/security-review.md`: mark CR1 / SV-1 fixed (in the summary table at ~line 1785 and in the finding's heading/status), naming the test `sync::tests::a_spliced_item_is_refused`. Whole-item replay of an older version stays as recorded (#9, S5, T1b).
- `docs/threat-model.md`: where T1b (replay) is described, add that a splice across versions is refused by the binding.

- [ ] **Step 7: Commit**

```bash
git add crates/havenkeys-core docs/crypto.md docs/security-review.md docs/threat-model.md
git commit -m "fix(core): bind an item's details blob to its overview blob (CR1)"
```

---

### Task 2: Byte-budgeted pull and fetch on the server (SV-2, SV-3 paging)

**Files:**
- Modify: `crates/havenkeys-server/src/limits.rs`
- Modify: `crates/havenkeys-server/src/routes/sync.rs`
- Modify: `crates/havenkeys-server/src/routes/items.rs` (`fetch`, ~line 237)
- Test: `crates/havenkeys-server/src/routes/sync.rs` (unit), `crates/havenkeys-server/tests/sync.rs` (integration; needs Postgres — `scripts/test-server.sh`, or the container already on port 5433 with `HAVENKEYS_TEST_DATABASE_URL`)

**Interfaces:**
- Produces:
  - `pub const MAX_PAGE_BYTES: usize = 12 * 1024 * 1024;` (limits.rs)
  - `pub(crate) const ROW_JSON_BYTES: usize = 256;`, `pub(crate) fn b64_len(n: i64) -> usize`, `pub(crate) fn page_len(rows: &[(i64, usize)], window_full: bool) -> usize` (routes/sync.rs)
  - Fetch answer: `{"changes": [...], "unanswered": [uuid, ...]}` — `unanswered` lists requested items that exist in the vault but did not fit; it is always present (possibly empty).

- [ ] **Step 1: Write the failing unit tests**

Append to `crates/havenkeys-server/src/routes/sync.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::{MAX_PAGE_BYTES, MAX_PULL_PAGE};

    const MB: usize = 1024 * 1024;

    #[test]
    fn b64_len_rounds_up_to_whole_quads() {
        assert_eq!(b64_len(0), 0);
        assert_eq!(b64_len(1), 4);
        assert_eq!(b64_len(3), 4);
        assert_eq!(b64_len(4), 8);
        assert_eq!(b64_len(-5), 0);
    }

    #[test]
    fn small_rows_page_by_count_at_revision_boundaries() {
        let mut rows: Vec<(i64, usize)> = (0..400).map(|_| (1, 100)).collect();
        rows.extend((0..400).map(|_| (2, 100)));
        // 800 rows, window not full: the first revision fits, the second
        // would pass MAX_PULL_PAGE, so the page ends after revision 1.
        assert_eq!(page_len(&rows, false), 400);
        let few = vec![(1, 100), (2, 100), (2, 100)];
        assert_eq!(page_len(&few, false), 3);
        assert_eq!(page_len(&[], false), 0);
        assert!(MAX_PULL_PAGE >= 400);
    }

    #[test]
    fn big_rows_page_by_bytes() {
        let rows = vec![(1, 5 * MB), (2, 5 * MB), (3, 5 * MB)];
        // 10 MiB fits, 15 MiB does not.
        assert_eq!(page_len(&rows, false), 2);
        assert!(2 * 5 * MB <= MAX_PAGE_BYTES);
    }

    #[test]
    fn page_len_always_takes_the_first_revision() {
        let rows = vec![(1, 15 * MB), (2, 1)];
        assert_eq!(page_len(&rows, false), 1);
    }

    #[test]
    fn a_full_window_never_ends_on_its_last_revision() {
        // The window may have cut revision 2 short: it is left for the
        // next page even though it would fit.
        let rows = vec![(1, 10), (2, 10), (2, 10)];
        assert_eq!(page_len(&rows, true), 1);
    }
}
```

Run: `cargo test -p havenkeys-server --lib routes::sync`
Expected: compile errors (`b64_len`, `page_len`, `MAX_PAGE_BYTES` not found).

- [ ] **Step 2: Implement the paging**

`crates/havenkeys-server/src/limits.rs`:

```rust
/// Pull and fetch answers stop adding rows past this many bytes (blobs as
/// base64, plus a per-row allowance for the JSON around them), so every
/// answer stays under the client's 17 MiB cap. A page always holds at
/// least one whole revision, which `MAX_BODY_BYTES` already bounds.
pub const MAX_PAGE_BYTES: usize = 12 * 1024 * 1024;
```

`crates/havenkeys-server/src/routes/sync.rs`: replace `pull` and `trim_to_revision_boundary` with the following (keep `change_json` as it is; update the module doc comment to say pages are cut by row count *and* bytes, at revision boundaries, and that blobs are read only for the rows that go out):

```rust
use crate::limits::{MAX_CHANGES_PER_BATCH, MAX_PAGE_BYTES, MAX_PULL_PAGE};

/// The JSON around one change's blobs: id, revision, flags, quotes.
pub(crate) const ROW_JSON_BYTES: usize = 256;

/// Length of `n` bytes as base64.
pub(crate) fn b64_len(n: i64) -> usize {
    usize::try_from(n).unwrap_or(0).div_ceil(3) * 4
}

/// How many of `rows` — `(revision, answer bytes)`, ordered by revision —
/// make one page: whole revisions only, at most `MAX_PULL_PAGE` rows and
/// `MAX_PAGE_BYTES` bytes, but always the first revision. When the query
/// window was full its last revision may be cut short, so it never ends a
/// page.
pub(crate) fn page_len(rows: &[(i64, usize)], window_full: bool) -> usize {
    let complete = match rows.last() {
        Some(&(last, _)) if window_full => rows.iter().position(|r| r.0 == last).unwrap_or(0),
        _ => rows.len(),
    };
    let (mut end, mut bytes) = (0usize, 0usize);
    while end < complete {
        let revision = rows[end].0;
        let next = end + rows[end..].iter().take_while(|r| r.0 == revision).count();
        let more: usize = rows[end..next].iter().map(|r| r.1).sum();
        if end > 0 && (next > MAX_PULL_PAGE as usize || bytes + more > MAX_PAGE_BYTES) {
            break;
        }
        end = next;
        bytes += more;
    }
    end
}

pub async fn pull(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<PullQuery>,
) -> Result<axum::Json<serde_json::Value>, ApiError> {
    if query.since < 0 {
        return Err(ApiError::InvalidRequest("since is not valid"));
    }
    let db = state.pool.get().await?;
    let window = MAX_PULL_PAGE + MAX_CHANGES_PER_BATCH as i64 + 1;
    // Sizes first: the blobs are read only for the rows that go out.
    let sized: Vec<(i64, usize)> = db
        .query(
            "SELECT revision,
                    COALESCE(octet_length(overview), 0)::bigint,
                    COALESCE(octet_length(details), 0)::bigint
               FROM items
              WHERE vault_id = $1 AND revision > $2
              ORDER BY revision, item_id
              LIMIT $3",
            &[&session.vault_id, &query.since, &window],
        )
        .await?
        .iter()
        .map(|r| (r.get(0), b64_len(r.get(1)) + b64_len(r.get(2)) + ROW_JSON_BYTES))
        .collect();
    let end = page_len(&sized, sized.len() as i64 == window);
    let has_more = end < sized.len();

    // Rows rewritten since the size query moved past every revision here
    // and come in a later page; nothing can move into this range.
    let (changes, cursor) = match end.checked_sub(1).map(|last| sized[last].0) {
        Some(through) => {
            let rows = db
                .query(
                    "SELECT item_id, revision, overview, details, deleted_at IS NOT NULL
                       FROM items
                      WHERE vault_id = $1 AND revision > $2 AND revision <= $3
                      ORDER BY revision, item_id",
                    &[&session.vault_id, &query.since, &through],
                )
                .await?;
            (rows.iter().map(change_json).collect::<Vec<_>>(), through)
        }
        // With no rows left, the cursor jumps to the vault's own revision:
        // an idle client converges instead of asking for the same range
        // forever.
        None => (
            Vec::new(),
            db.query_one(
                "SELECT revision FROM vaults WHERE id = $1",
                &[&session.vault_id],
            )
            .await?
            .get(0),
        ),
    };

    Ok(axum::Json(serde_json::json!({
        "cursor": cursor,
        "hasMore": has_more,
        "changes": changes,
    })))
}
```

`crates/havenkeys-server/src/routes/items.rs` `fetch` — after the existing validation:

```rust
    let db = state.pool.get().await?;
    let sized = db
        .query(
            "SELECT item_id,
                    COALESCE(octet_length(overview), 0)::bigint,
                    COALESCE(octet_length(details), 0)::bigint
               FROM items WHERE vault_id = $1 AND item_id = ANY($2)
              ORDER BY item_id",
            &[&session.vault_id, &req.item_ids],
        )
        .await?;
    // As many as fit, at least one; the rest are named, so the client asks
    // again rather than reading them as deleted.
    let mut answered: Vec<Uuid> = Vec::new();
    let mut bytes = 0usize;
    for row in &sized {
        let more = b64_len(row.get(1)) + b64_len(row.get(2)) + ROW_JSON_BYTES;
        if !answered.is_empty() && bytes + more > MAX_PAGE_BYTES {
            break;
        }
        answered.push(row.get(0));
        bytes += more;
    }
    let unanswered: Vec<Uuid> = sized[answered.len()..].iter().map(|r| r.get(0)).collect();
    let rows = db
        .query(
            "SELECT item_id, revision, overview, details, deleted_at IS NOT NULL
               FROM items WHERE vault_id = $1 AND item_id = ANY($2)
              ORDER BY item_id",
            &[&session.vault_id, &answered],
        )
        .await?;
    let changes: Vec<serde_json::Value> = rows.iter().map(change_json).collect();
    Ok(axum::Json(serde_json::json!({ "changes": changes, "unanswered": unanswered })))
```

with `use crate::limits::MAX_PAGE_BYTES;` and `use crate::routes::sync::{b64_len, ROW_JSON_BYTES};` (make the `sync` module path match how `change_json` is already imported in `items.rs`). Update `fetch`'s doc comment to mention `unanswered`.

Run: `cargo test -p havenkeys-server --lib routes::sync` → PASS.

- [ ] **Step 3: Write the integration tests**

Append to `crates/havenkeys-server/tests/sync.rs` (helpers `support::change`, `support::write`, `support::pull`, local `fetch` exist):

```rust
/// Six items of 2 × 3 MiB blobs (about 8 MiB of base64 each), one per batch.
async fn big_items(server: &support::TestServer, sess: &support::Sess) -> Vec<Uuid> {
    let blob = vec![7u8; 3 * 1024 * 1024];
    let mut ids = Vec::new();
    for _ in 0..6 {
        let id = Uuid::new_v4();
        let (status, _) = support::write(server, sess, vec![support::change(id, None, &blob, &blob)]).await;
        assert_eq!(status, 200);
        ids.push(id);
    }
    ids
}

#[tokio::test]
async fn a_vault_of_big_items_pulls_in_pages_under_the_cap() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "big@example.com").await;
    let ids = big_items(&server, &sess).await;

    let mut seen = std::collections::HashSet::new();
    let mut cursor = 0;
    loop {
        let res = server
            .get_as(&format!("/v1/sync?since={cursor}"), &sess)
            .send()
            .await
            .unwrap();
        let bytes = res.bytes().await.unwrap();
        assert!(bytes.len() < 17 * 1024 * 1024, "page of {} bytes", bytes.len());
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        let changes = body["changes"].as_array().unwrap();
        assert!(changes.len() <= 1, "two 8 MiB items do not fit one 12 MiB page");
        for change in changes {
            seen.insert(change["itemId"].as_str().unwrap().to_string());
        }
        let next = body["cursor"].as_i64().unwrap();
        let more = body["hasMore"].as_bool().unwrap();
        assert!(next > cursor || !more, "no progress");
        cursor = next;
        if !more {
            break;
        }
    }
    assert_eq!(seen.len(), ids.len());
    server.cleanup().await;
}

#[tokio::test]
async fn a_fetch_of_big_items_names_what_did_not_fit() {
    let server = support::TestServer::start().await;
    let (_, sess) = support::signed_in(&server, "big-fetch@example.com").await;
    let ids = big_items(&server, &sess).await;
    let unknown = Uuid::new_v4();
    let mut asked = ids.clone();
    asked.push(unknown);

    let (status, body) = fetch(&server, &sess, &asked).await;
    assert_eq!(status, 200);
    let changes = body["changes"].as_array().unwrap();
    let unanswered: Vec<String> = body["unanswered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(changes.len(), 1);
    assert_eq!(unanswered.len(), ids.len() - 1);
    // An item not in the vault is neither answered nor unanswered.
    assert!(!unanswered.contains(&unknown.to_string()));
    server.cleanup().await;
}
```

Also extend `a_fetch_returns_current_rows_and_tombstones` with `assert_eq!(body["unanswered"], json!([]));`.

Run: `cargo test -p havenkeys-server --test sync` (Postgres needed).
Expected: PASS, including the existing `a_page_never_cuts_a_batch_in_half` and `a_pull_pages_and_the_cursor_carries_on`.

- [ ] **Step 4: Documentation and commit**

- `docs/server-sync.md`: where pull paging and fetch are described, state the byte budget (12 MiB, whole revisions, always at least one) and the fetch `unanswered` field.
- `docs/security-review.md`: SV-2 fixed (server half here, client half in Task 3 — mark it fixed after Task 3 lands, i.e. write "fixed" now and name both tests); SV-3: paging and the server's memory use per request fixed; the per-account storage quota stays open (planned).

```bash
git add crates/havenkeys-server docs/server-sync.md docs/security-review.md
git commit -m "fix(server): cut pull and fetch answers by bytes, reading only the blobs that go out (SV-2)"
```

---

### Task 3: The client asks again for unanswered items (SV-2)

**Files:**
- Modify: `crates/havenkeys-sync-client/src/wire.rs` (`FetchDto`, ~line 269)
- Modify: `crates/havenkeys-sync-client/src/client.rs` (`fetch_items`, ~line 313)
- Modify: `crates/havenkeys-sync-client/src/transport.rs:16` (comment)
- Modify: `crates/havenkeys-client/src/sync.rs` (refetch loop, ~line 175)
- Test: `crates/havenkeys-sync-client/tests/hostile.rs`, `crates/havenkeys-sync-client/tests/round_trip.rs` (only if it calls `fetch_items` — adapt to the new return type)

**Interfaces:**
- Consumes: Task 2's fetch answer `{"changes": [...], "unanswered": [...]}`.
- Produces:
  - `pub struct Fetched { pub changes: Vec<RemoteChange>, pub unanswered: Vec<Uuid> }` (exported from `havenkeys_sync_client` beside `Pulled`)
  - `pub async fn SyncClient::fetch_items(&self, session: &Session, ids: &[Uuid]) -> Result<Fetched>` (the client type keeps its current name)

- [ ] **Step 1: Write the failing tests**

Change the last assertion of `a_fetch_answer_is_bounded_to_what_was_asked` in `crates/havenkeys-sync-client/tests/hostile.rs` to read `.changes.len()` instead of `.len()`, and append:

```rust
#[tokio::test]
async fn unanswered_items_must_be_asked_for_and_not_also_answered() {
    let asked = Uuid::from_u128(1);
    let later = Uuid::from_u128(3);
    let other = Uuid::from_u128(2);

    let body = format!(r#"{{"changes":[],"unanswered":["{other}"]}}"#);
    let err = Stub::ok(&body).fetch_items(&session(), &[asked]).await.unwrap_err();
    assert!(matches!(err, SyncError::Protocol(_)));

    let body = format!(
        r#"{{"changes":[{{"itemId":"{asked}","revision":1,"deleted":true}}],"unanswered":["{asked}"]}}"#
    );
    let err = Stub::ok(&body).fetch_items(&session(), &[asked]).await.unwrap_err();
    assert!(matches!(err, SyncError::Protocol(_)));

    let body = format!(
        r#"{{"changes":[{{"itemId":"{asked}","revision":1,"deleted":true}}],"unanswered":["{later}"]}}"#
    );
    let fetched = Stub::ok(&body).fetch_items(&session(), &[asked, later]).await.unwrap();
    assert_eq!(fetched.changes.len(), 1);
    assert_eq!(fetched.unanswered, vec![later]);

    // An answer from a server without the field still parses.
    let body = format!(r#"{{"changes":[{{"itemId":"{asked}","revision":1,"deleted":true}}]}}"#);
    assert!(Stub::ok(&body).fetch_items(&session(), &[asked]).await.unwrap().unanswered.is_empty());
}
```

For the client loop, add to the `tests` module of `crates/havenkeys-client/src/sync.rs` only if that module already has a test server that serves `/v1/items/fetch` (search for `items/fetch` in `crates/havenkeys-client/src`). If it does not, the loop is covered by the server round trip below and by reading; do not build a new test server for it. The required behaviour, `unanswered_items_are_asked_again_and_never_deleted`, is pinned at the core level instead — append to `crates/havenkeys-core/src/sync.rs` tests:

```rust
    #[test]
    fn unanswered_items_are_asked_again_and_never_deleted() {
        // The client passes only the answered ids to apply_refetched; an
        // unreadable item it did not pass stays recorded, not deleted.
        let mut v = activated_vault();
        let a = Uuid::from_u128(0xa);
        let b = Uuid::from_u128(0xb);
        let junk = |id: Uuid, revision: i64| RemoteChange {
            item_id: id,
            revision,
            overview: Some(vec![1; 40]),
            details: Some(vec![2; 40]),
            deleted: false,
        };
        let pulled = v.apply_remote_changes(2, vec![junk(a, 1), junk(b, 2)], NOW).unwrap();
        assert_eq!(pulled.skipped_items, 2);
        let mut pending = v.unreadable_item_ids().unwrap();
        pending.sort();
        assert_eq!(pending, vec![a, b]);
        // Only `a` was answered (here: by a tombstone).
        let tomb = RemoteChange { item_id: a, revision: 3, overview: None, details: None, deleted: true };
        v.apply_refetched(&[a], vec![tomb], NOW).unwrap();
        assert_eq!(v.unreadable_item_ids().unwrap(), vec![b]);
    }
```

Run: `cargo test -p havenkeys-sync-client --test hostile` and `cargo test -p havenkeys-core --lib unanswered`
Expected: the hostile tests fail to compile (`Fetched` fields); the core test passes already (it pins the contract the client loop relies on — keep it).

- [ ] **Step 2: Implement**

`crates/havenkeys-sync-client/src/wire.rs`:

```rust
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchDto {
    pub changes: Vec<RemoteChangeDto>,
    /// Asked-for items that exist but did not fit this answer.
    #[serde(default)]
    pub unanswered: Vec<Uuid>,
}
```

`crates/havenkeys-sync-client/src/client.rs` (define `Fetched` next to `Pulled` and re-export it where `Pulled` is re-exported in `lib.rs`):

```rust
/// A fetch answer: the items that fit, and the asked-for items that did not
/// (ask for those again; they are not deleted).
#[derive(Debug)]
pub struct Fetched {
    pub changes: Vec<RemoteChange>,
    pub unanswered: Vec<Uuid>,
}

    /// The current version of specific items, for retrying ones that did not
    /// open. Every returned change must be for an ID that was asked for, at
    /// most once, and an unanswered ID must have been asked for and not also
    /// answered.
    pub async fn fetch_items(&self, session: &Session, ids: &[Uuid]) -> Result<Fetched> {
        if ids.is_empty() || ids.len() > MAX_CHANGES {
            return Err(SyncError::Refused("item list is not valid"));
        }
        let response = self
            .post(
                "/v1/items/fetch",
                Some(session),
                &wire::FetchBody { item_ids: ids },
            )
            .await?;
        let dto: wire::FetchDto = expect_ok(response)?;
        let asked: std::collections::HashSet<Uuid> = ids.iter().copied().collect();
        let mut seen = std::collections::HashSet::new();
        for id in dto.changes.iter().map(|c| c.item_id).chain(dto.unanswered.iter().copied()) {
            if !asked.contains(&id) || !seen.insert(id) {
                return Err(SyncError::Protocol("an item that was not asked for"));
            }
        }
        Ok(Fetched {
            changes: dto
                .changes
                .into_iter()
                .map(RemoteChange::try_from)
                .collect::<Result<Vec<_>>>()?,
            unanswered: dto.unanswered,
        })
    }
```

`crates/havenkeys-sync-client/src/transport.rs:16`: correct the comment on `MAX_RESPONSE_BYTES` — the server keeps pull and fetch answers under `MAX_PAGE_BYTES` (12 MiB) and a single revision under `MAX_BODY_BYTES` (16 MiB), so 17 MiB leaves room for one whole write batch.

`crates/havenkeys-client/src/sync.rs`, replacing the refetch loop in `sync_now`:

```rust
        // Items that did not open earlier are asked for again, by id, until
        // they open, are deleted, or a Re-download clears them. Best effort:
        // a failure stops the retry for this sync and nothing more. Only a
        // refused session is acted on. Items the server could not fit in one
        // answer are asked for again, never read as deleted.
        let pending = self.vault()?.unreadable_item_ids()?;
        'chunks: for chunk in pending.chunks(MAX_BATCH) {
            let mut ask = chunk.to_vec();
            while !ask.is_empty() {
                let fetched = match server.fetch_items(&session, &ask).await {
                    Ok(fetched) => fetched,
                    Err(SyncError::Unauthorized) => {
                        let _ = self.failed(SyncError::Unauthorized);
                        break 'chunks;
                    }
                    Err(_) => break 'chunks,
                };
                let answered: Vec<Uuid> = ask
                    .iter()
                    .copied()
                    .filter(|id| !fetched.unanswered.contains(id))
                    .collect();
                // A server that answers nothing would spin this loop.
                if answered.is_empty() {
                    break 'chunks;
                }
                let applied = self
                    .vault()
                    .and_then(|mut v| Ok(v.apply_refetched(&answered, fetched.changes, now_ms())?));
                let Ok(page) = applied else { break 'chunks };
                report.added += page.added;
                report.updated += page.updated;
                report.deleted += page.deleted;
                ask = fetched.unanswered;
            }
        }
```

(add `use uuid::Uuid;` if not already imported). Update any other caller the compiler reports (`crates/havenkeys-sync-client/tests/round_trip.rs`) to use `.changes`.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p havenkeys-sync-client && cargo test -p havenkeys-client && cargo test -p havenkeys-core && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS. Then, with Postgres: `cargo test -p havenkeys-sync-client --test round_trip` and `cargo test -p havenkeys-server --test sync` → PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/havenkeys-sync-client crates/havenkeys-client crates/havenkeys-core/src/sync.rs
git commit -m "fix(sync): ask again for items a fetch answer could not fit (SV-2)"
```

---

### Task 4: Popup Fill refuses opaque-origin documents (EX-01)

**Files:**
- Modify: `apps/extension/src/content/index.ts` (`handleFill`, ~line 507)
- Modify: `apps/extension/src/webauthn/bridge.ts` (request listener, ~line 171)
- Test: `apps/extension/src/content/content.test.ts`, `apps/extension/src/webauthn/bridge.test.ts`
- Modify: `docs/native-messaging.md:449`, `docs/autofill.md` (where popup fill is described, if it states origin checks), `docs/security-review.md`

**Interfaces:**
- Produces: `handleFill` returns `{ filled: 0, pressing: null }` when `self.origin !== location.origin`; the passkey bridge answers `{ outcome: "fallback" }` for every request in such a document.

- [ ] **Step 1: Write the failing tests**

In `apps/extension/src/content/content.test.ts`, inside `describe("content script", …)` next to "fills the page's login form for a popup fill on the matched origin":

```ts
  it("refuses_a_popup_fill_into_an_opaque_origin_document", () => {
    // A document served with `Content-Security-Policy: sandbox` keeps its
    // URL (and location.origin) but runs with an opaque origin.
    const real = Object.getOwnPropertyDescriptor(window, "origin");
    Object.defineProperty(window, "origin", { value: "null", configurable: true });
    try {
      expect(deliver(loginFill(location.origin))).toEqual({ filled: 0, pressing: null });
      expect(field("pw").value).toBe("");
    } finally {
      if (real) Object.defineProperty(window, "origin", real);
      else delete (window as { origin?: string }).origin;
    }
    // The same page without the sandbox still fills.
    expect(deliver(loginFill(location.origin))).toEqual({ filled: 2, pressing: null });
  });
```

In `apps/extension/src/webauthn/bridge.test.ts`, inside `describe("isolated bridge", …)`:

```ts
  it("falls back without asking the background in an opaque-origin document", async () => {
    const real = Object.getOwnPropertyDescriptor(window, "origin");
    Object.defineProperty(window, "origin", { value: "null", configurable: true });
    try {
      reply = () => ({ ok: true, token: TOKEN, ui: "chooser" });
      request({ kind: "get", id: hex(77), options: get });
      await flush();
      expect(sent).toEqual([]);
      expect(responses).toEqual([{ id: hex(77), outcome: "fallback" }]);
    } finally {
      if (real) Object.defineProperty(window, "origin", real);
      else delete (window as { origin?: string }).origin;
    }
  });
```

(If jsdom defines `origin` on `Window.prototype` rather than the instance, the `getOwnPropertyDescriptor` is undefined and the `delete` restores the prototype's getter — which is what the `finally` does.)

Run: `cd apps/extension && npx vitest run src/content/content.test.ts src/webauthn/bridge.test.ts`
Expected: both new tests FAIL (the fill happens; the bridge asks the background).

- [ ] **Step 2: Implement**

`apps/extension/src/content/index.ts`, at the top of `handleFill`:

```ts
  function handleFill(m: Extract<BackgroundToContent, { type: "bg_fill" }>): FillReply {
    const none: FillReply = { filled: 0, pressing: null };
    // A sandboxed document (CSP `sandbox`) keeps its URL but runs with an
    // opaque origin: page script there is not the site and must get nothing.
    if (self.origin !== location.origin) return none;
    // The frame may have navigated since the desktop matched its URL.
    if (m.origin !== location.origin) return none;
```

`apps/extension/src/webauthn/bridge.ts`, in the `REQUEST_EVENT` listener:

```ts
  window.addEventListener(REQUEST_EVENT, (e) => {
    const req = parsePageRequest((e as CustomEvent).detail);
    if (!req) return;
    if (req.kind === "cancel") return cancel(req.id);
    // Synchronous: the page script knows at once that a bridge is here.
    dispatch({ id: req.id, outcome: "ack" });
    // An opaque-origin (sandboxed) document is not the site; the background
    // refuses it too, but nothing is sent from here at all.
    if (self.origin !== location.origin) return dispatch({ id: req.id, outcome: "fallback" });
    void begin(req);
  });
```

Run the two test files again → PASS. Then the whole suite: `cd apps/extension && npm test` (and `npm run typecheck`, `npm run lint` if defined in `package.json`) → PASS.

- [ ] **Step 3: Documentation and commit**

- `docs/native-messaging.md:449`: replace the unqualified "A sandboxed frame (origin `null`) is ignored." with: in-page requests from a sandboxed frame are refused by the background (the browser reports origin `null`); a popup fill and the passkey bridge also refuse a document whose `self.origin` is opaque, since `tab.url` still names the real site.
- `docs/security-review.md`: EX-01 fixed (summary table ~line 1790 and the finding), naming both tests.

```bash
git add apps/extension/src/content apps/extension/src/webauthn docs/native-messaging.md docs/security-review.md
git commit -m "fix(extension): never fill or bridge passkeys in an opaque-origin document (EX-01)"
```

---

### Task 5: Windows: the native host refuses a pipe served by another user (BR-1/P10)

**Files:**
- Modify: `crates/havenkeys-protocol/Cargo.toml`
- Create: `crates/havenkeys-protocol/src/win_identity.rs`
- Modify: `crates/havenkeys-protocol/src/lib.rs` (declare the module, Windows only)
- Modify: `crates/havenkeys-protocol/src/endpoint.rs` (Windows `connect`, module doc)
- Modify: `crates/havenkeys-bridge/src/server.rs` (Windows `peer_is_same_user`, ~line 372)
- Modify: `docs/native-messaging.md` (pipe-squatting caveat), `docs/security-review.md` (BR-1, P10), `docs/threat-model.md` if it repeats P10

**Interfaces:**
- Produces (Windows only): `pub fn havenkeys_protocol::win_identity::process_is_current_user(pid: u32) -> std::io::Result<bool>`; `Endpoint::connect` returns `Err(PermissionDenied)` when the pipe's server process runs as another user.

- [ ] **Step 1: Add the dependency and the identity check**

`crates/havenkeys-protocol/Cargo.toml`, under `[target.'cfg(windows)'.dependencies]`:

```toml
# Pipe peer identity (BR-1): the peer process's token user.
windows-sys = { version = "0.61", features = ["Win32_Foundation", "Win32_Security", "Win32_System_Threading"] }
```

`crates/havenkeys-protocol/src/lib.rs`: add

```rust
#[cfg(windows)]
pub mod win_identity;
```

Create `crates/havenkeys-protocol/src/win_identity.rs`:

```rust
//! Windows: does a process run as the current user? Used on both ends of the
//! bridge pipe, because another account can create the pipe name before
//! HavenKeys does (security-review BR-1). Compares token *user* SIDs, so an
//! elevated desktop app and an unelevated native host still match. Any
//! failure — including a process we may not open, as another user's is —
//! answers `Err`, which callers treat as "not us".

use std::io;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{EqualSid, GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

struct Owned(HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: a handle this module opened and still owns.
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// The process's `TOKEN_USER`, in a buffer aligned for it.
fn token_user(process: HANDLE) -> io::Result<Vec<u64>> {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: `process` is a valid process handle; `token` receives a new handle.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Owned(token);
    let mut len = 0u32;
    // SAFETY: a size query with no buffer; it fails and sets `len`.
    unsafe { GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut len) };
    if len == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: `buf` holds at least `len` bytes, aligned for TOKEN_USER.
    if unsafe { GetTokenInformation(token.0, TokenUser, buf.as_mut_ptr().cast(), len, &mut len) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(buf)
}

pub fn process_is_current_user(pid: u32) -> io::Result<bool> {
    // SAFETY: plain call; a null result is checked.
    let peer = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if peer.is_null() {
        return Err(io::Error::last_os_error());
    }
    let peer = Owned(peer);
    let theirs = token_user(peer.0)?;
    // SAFETY: the current-process pseudo handle needs no closing.
    let ours = token_user(unsafe { GetCurrentProcess() })?;
    let sid = |b: &[u64]| {
        // SAFETY: `b` was filled by GetTokenInformation(TokenUser); the SID
        // it points to lives inside the same buffer.
        unsafe { (*b.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    };
    // SAFETY: two valid SIDs, each inside its live buffer.
    Ok(unsafe { EqualSid(sid(&theirs), sid(&ours)) } != 0)
}
```

If `cargo check --target x86_64-pc-windows-gnu -p havenkeys-protocol` reports a type mismatch with windows-sys 0.61 (for example `HANDLE` or the `BOOL` argument of `OpenProcess`), adapt to the exact signature it names; keep the logic.

- [ ] **Step 2: Check the peer on both ends**

`crates/havenkeys-protocol/src/endpoint.rs`, Windows `connect`:

```rust
    /// Connect as a client, then make sure the pipe's server runs as us:
    /// another account can create this pipe name first (BR-1), and the
    /// DACL protects only a pipe HavenKeys itself created.
    pub fn connect(&self) -> io::Result<Stream> {
        use interprocess::local_socket::traits::StreamCommon;
        let stream = Stream::connect(self.name()?)?;
        let ours = stream
            .peer_creds()
            .ok()
            .and_then(|c| c.pid())
            .is_some_and(|pid| crate::win_identity::process_is_current_user(pid).unwrap_or(false));
        if !ours {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the bridge pipe belongs to another user",
            ));
        }
        Ok(stream)
    }
```

(If `peer_creds` is reachable without the `StreamCommon` import, drop the import; for `local_socket::Stream` on Windows `peer_creds().pid()` is the peer's process ID — the server's, from the client side.) Update the module doc's Windows bullet: the client checks the pipe server's user before using it; replace "See docs/native-messaging.md for the pipe-squatting caveat." accordingly.

`crates/havenkeys-bridge/src/server.rs`:

```rust
// Named pipes: the DACL admits only the owner; the client's user is checked
// too, in case the pipe is not the one HavenKeys created.
#[cfg(windows)]
fn peer_is_same_user(stream: &Stream) -> bool {
    stream
        .peer_creds()
        .ok()
        .and_then(|c| c.pid())
        .is_some_and(|pid| havenkeys_protocol::win_identity::process_is_current_user(pid).unwrap_or(false))
}
```

- [ ] **Step 3: Verify**

Run:

```bash
cargo check --target x86_64-pc-windows-gnu -p havenkeys-protocol -p havenkeys-bridge -p havenkeys-native-host
cargo clippy --target x86_64-pc-windows-gnu -p havenkeys-protocol -p havenkeys-bridge -p havenkeys-native-host -- -D warnings
cargo test -p havenkeys-protocol -p havenkeys-bridge -p havenkeys-native-host
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all pass (the Windows build is checked, not run; the Linux tests are unchanged). If the Windows check fails on a crate other than these three for reasons unrelated to this change, report it and check only these three.

- [ ] **Step 4: Documentation and commit**

- `docs/native-messaging.md`: replace the pipe-squatting caveat with what now happens — the native host checks that the pipe's server process runs as the same Windows user (token user SID) and refuses it otherwise; the desktop app checks its client the same way; what remains: if another user squats the name first, HavenKeys cannot serve until that pipe goes away (availability, not confidentiality).
- `docs/security-review.md`: BR-1 and P10 fixed in code, "not yet verified on Windows" (add a manual check: with a second Windows account, create the pipe name first; the extension must report HavenKeys unreachable and send nothing).

```bash
git add crates/havenkeys-protocol crates/havenkeys-bridge docs/native-messaging.md docs/security-review.md docs/threat-model.md
git commit -m "fix(protocol): on Windows, refuse a bridge pipe served by another user (BR-1)"
```
