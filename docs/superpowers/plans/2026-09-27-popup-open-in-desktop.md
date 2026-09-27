# Edit a Login in the Desktop App from the Extension Popup — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A pencil icon, last on each login row of the extension's toolbar popup, that brings the HavenKeys desktop window forward with that login open in the editor.

**Architecture:** A new native-messaging request `open_item { itemId, url, topUrl? }` goes popup → background → native host → bridge. The bridge checks it like `fill_item` (unlocked, integration on, item matches the page, Secret-class rate limit), then calls a desktop hook outside the vault lock. The hook shows the window and emits `vault://open-item`. The React vault screen opens the editor, asking first if another edit has unsaved changes.

**Tech Stack:** Rust (havenkeys-protocol, havenkeys-bridge, Tauri 2 desktop), TypeScript (packages/protocol, MV3 extension, React desktop UI), vitest.

**Spec:** `docs/superpowers/specs/2026-09-27-popup-open-in-desktop-design.md`

## Global Constraints

- The request carries only `itemId`, `url`, `topUrl?`; the reply is `{"type":"open_item"}` with no other fields. No secret or item data crosses.
- The bridge refuses any item the page URL does not match, exactly like `fill_item`; an unknown ID answers `denied`, the same as a wrong site.
- The hook is called only after all checks pass, and never while the vault mutex is held.
- `open_item` is in `RequestClass::Secret`.
- Only the toolbar popup can trigger it: the background accepts `popup_open_item` only from the popup page (existing `isFromPopup` gate) and reads the URL from the active tab, never from the message.
- DOM in the popup is built with `createElement` / `createElementNS` only — no `innerHTML`.
- UI strings in `en` and `pt-BR` for both the desktop and the extension.
- Commit messages: conventional; **never** a Co-Authored-By trailer.
- Rust: `cargo fmt --check` and `cargo clippy --all-targets` clean for every crate touched.

## Review Focus

1. **Desktop window hidden in the tray** when the request arrives: expect the window to be shown and focused, then the editor opened. Pinned by the manual check in Task 5 (`show_main_window` is the existing tray path).
2. **The login is filtered out of the current list** (a search query, or the Secure Notes section) when the event arrives: expect the search to be cleared and the Logins section selected so the editor has its item. Pinned in Task 3 by `decideOpen` returning `open`, and by VaultScreen clearing `query` and setting `section` to `login`.
3. **Two quick clicks, or the same login already open in the editor:** expect no second editor and no confirm prompt. Pinned in Task 3 (`decideOpen` → `already`).
4. **A new, unsaved item** (pane `new`) with typed changes when the request arrives: expect the discard prompt too, not a silent loss. Pinned in Task 3 (`decideOpen` treats `new` like `edit`).
5. **A request for a login saved for a different site, or a secure note:** expect `denied` and no window movement. Pinned in Task 2 (hook-call count stays 0).

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/havenkeys-protocol/src/message.rs` | `Request::OpenItem`, `ResultBody::OpenItem` |
| `packages/protocol/src/index.ts` | TS mirror + result parser |
| `crates/havenkeys-bridge/src/{dispatch.rs,server.rs,ratelimit.rs}` | check, `Dispatched::OpenItem`, hook, rate class |
| `apps/desktop/src-tauri/src/{lib.rs,state.rs}` | set the hook: show window + emit event |
| `apps/desktop/src/lib/openItem.ts` (new) | pure `decideOpen`, `isDirty` |
| `apps/desktop/src/views/{VaultScreen,ItemEditor}.tsx`, `src/lib/api.ts`, i18n | listen, confirm, dirty reporting |
| `apps/extension/src/messaging/popup.ts`, `background/popup-handler.ts`, `popup/popup.ts`, `popup/popup.css`, i18n | message, handler, button |
| `docs/native-messaging.md`, `docs/security-model.md`, `docs/threat-model.md`, spec | docs |

---

### Task 1: Protocol — `open_item` request and result

**Files:**
- Modify: `crates/havenkeys-protocol/src/message.rs` (enum `Request` ~line 39, `Request::kind` ~173, `Request::urls` ~192, enum `ResultBody` ~419, its `kind` ~495)
- Modify: `packages/protocol/src/index.ts` (`Request` ~30, `Result` ~102, `parseResult` ~259)
- Test: `crates/havenkeys-protocol/tests/messages.rs`, `packages/protocol/src/index.test.ts`
- Modify: `docs/native-messaging.md` (request table ~line 113, example results ~153)

**Interfaces:**
- Produces: Rust `Request::OpenItem { item_id: Uuid, url: String, top_url: Option<String> }` (wire `{"type":"open_item","itemId","url","topUrl"?}`), `ResultBody::OpenItem {}` (wire `{"type":"open_item"}`); TS `Request` member `{ type: "open_item"; itemId: string; url: string; topUrl?: string }`, `Result` member `{ type: "open_item" }`.

- [ ] **Step 1: Failing Rust tests**

In `crates/havenkeys-protocol/tests/messages.rs`, add:

```rust
#[test]
fn open_item_parses_and_reencodes() {
    let s = format!(
        r#"{{"v":1,"id":3,"request":{{"type":"open_item","itemId":"{ITEM}","url":"https://github.com/","topUrl":"https://github.com/"}}}}"#
    );
    let env = parse(&s).unwrap();
    assert_eq!(
        env.request,
        Request::OpenItem {
            item_id: Uuid::parse_str(ITEM).unwrap(),
            url: "https://github.com/".into(),
            top_url: Some("https://github.com/".into()),
        }
    );
    assert_eq!(env.request.kind(), "open_item");
    let again = serde_json::to_vec(&env).unwrap();
    assert_eq!(parse_request(&again).unwrap(), env);
}

#[test]
fn malformed_open_item_is_rejected() {
    let long = "a".repeat(MAX_URL_BYTES + 1);
    for s in [
        r#"{"v":1,"id":1,"request":{"type":"open_item","itemId":"not-a-uuid","url":"https://a.com/"}}"#.to_owned(),
        format!(r#"{{"v":1,"id":1,"request":{{"type":"open_item","itemId":"{ITEM}"}}}}"#),
        format!(r#"{{"v":1,"id":1,"request":{{"type":"open_item","itemId":"{ITEM}","url":"https://a.com/","extra":1}}}}"#),
        format!(r#"{{"v":1,"id":1,"request":{{"type":"open_item","itemId":"{ITEM}","url":""}}}}"#),
        format!(r#"{{"v":1,"id":1,"request":{{"type":"open_item","itemId":"{ITEM}","url":"https://a.com/{long}"}}}}"#),
    ] {
        assert!(parse(&s).is_err(), "{s}");
    }
}
```

(If `MAX_URL_BYTES` is not exported by `havenkeys_protocol::*`, use the literal value it has in `message.rs` and say so in the report.)

Run: `cargo test -p havenkeys-protocol open_item`
Expected: compile error — no variant `OpenItem`.

- [ ] **Step 2: Implement in Rust**

In `enum Request`, after `PasskeyStatus { … }`:

```rust
    /// Bring the desktop window forward on `item_id`'s editor, only if it is
    /// a login saved for `url`. Returns nothing.
    OpenItem {
        item_id: Uuid,
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
```

In `Request::kind`: `Request::OpenItem { .. } => "open_item",`.
In `Request::urls`: add `| Request::OpenItem { url, top_url, .. }` to the arm that returns `[Some(url), top_url.as_deref()]`.
In `enum ResultBody`, after the last variant: `OpenItem {},` and in its `kind`: `ResultBody::OpenItem {} => "open_item",`.
Fix any other exhaustive `match` the compiler reports in this crate the same way (URL-bearing request → treat like `FillItem`).

Run: `cargo test -p havenkeys-protocol` → all pass.

- [ ] **Step 3: Failing TS test**

In `packages/protocol/src/index.test.ts`, in "accepts every valid message shape" add
`{ v: 1, id: 13, result: { type: "open_item" } },`
and in "rejects anything else" add
`{ v: 1, id: 1, result: { type: "open_item", itemId: ID } },`.

Run: `pnpm --filter @havenkeys/protocol test` → the accept case fails.

- [ ] **Step 4: Implement in TS**

`Request` union: `| { type: "open_item"; itemId: string; url: string; topUrl?: string }` after `fill_item`.
`Result` union: `| { type: "open_item" }` at the end.
`parseResult`: `case "open_item": return hasExactKeys(v, ["type"]) ? { type: "open_item" } : null;`

Run: `pnpm --filter @havenkeys/protocol test` → pass; `pnpm --filter @havenkeys/protocol exec tsc --noEmit` (or the package's typecheck script) → clean.

- [ ] **Step 5: Docs**

`docs/native-messaging.md` request table, after `get_totp`:
`| \`open_item\` | \`itemId\` (UUID), \`url\`, \`topUrl\`? | yes | secret |`
and in the example results block add:
`{"v":1,"id":10,"result":{"type":"open_item"}}`
plus one sentence under the table: "`open_item` shows the desktop window with that login open in the editor; the item must be saved for `url`, like `fill_item`. Nothing is returned."

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-protocol packages/protocol docs/native-messaging.md
git commit -m "feat(protocol): open_item request"
```

---

### Task 2: Bridge — check, rate class, hook

**Files:**
- Modify: `crates/havenkeys-bridge/src/dispatch.rs` (`enum Dispatched` ~91, `dispatch` match)
- Modify: `crates/havenkeys-bridge/src/server.rs` (`Inner` ~39, `with_writer` ~101, `handle` ~139-190)
- Modify: `crates/havenkeys-bridge/src/ratelimit.rs:17` (doc comment of `Secret`)
- Test: `crates/havenkeys-bridge/tests/bridge.rs`

**Interfaces:**
- Consumes: `Request::OpenItem`, `ResultBody::OpenItem {}` (Task 1).
- Produces: `Bridge::set_open_item_hook(&self, hook: impl Fn(Uuid) + Send + Sync + 'static)`; `Dispatched::OpenItem(Uuid)`.

- [ ] **Step 1: Failing tests**

In `tests/bridge.rs`:

Add `opened: Arc<Mutex<Vec<Uuid>>>,` to `struct Fixture`. In `build_fixture`, after the `bridge` is built:

```rust
    let opened = Arc::new(Mutex::new(Vec::new()));
    let o2 = opened.clone();
    bridge.set_open_item_hook(move |id| o2.lock().unwrap().push(id));
```

and add `opened,` to the returned `Fixture`. Add the helper and tests:

```rust
fn open(f: &Fixture, id: Uuid, url: &str) -> serde_json::Value {
    call(f, serde_json::json!({"type": "open_item", "itemId": id, "url": url}))
}

#[test]
fn open_item_opens_a_login_saved_for_the_page() {
    let f = fixture();
    let r = open(&f, f.github, "https://github.com/login");
    assert_eq!(r["result"], serde_json::json!({"type": "open_item"}));
    assert_eq!(*f.opened.lock().unwrap(), vec![f.github]);
}

#[test]
fn open_item_is_origin_bound() {
    for url in ["https://evil.com/", "https://github.com.evil.com/", "https://evilgithub.com/", "not a url"] {
        let f = fixture();
        assert_eq!(error_code(&open(&f, f.github, url)), Some("denied"), "{url}");
        assert!(f.opened.lock().unwrap().is_empty(), "{url}");
    }
    let f = fixture();
    assert_eq!(error_code(&open(&f, f.bank, "https://github.com/")), Some("denied"));
    assert_eq!(error_code(&open(&f, f.note, "https://github.com/")), Some("denied"));
    assert_eq!(error_code(&open(&f, Uuid::new_v4(), "https://github.com/")), Some("denied"));
    assert!(f.opened.lock().unwrap().is_empty());
}

#[test]
fn open_item_refuses_when_locked_or_disabled() {
    let f = fixture();
    f.vault.lock().unwrap().lock();
    assert_eq!(error_code(&open(&f, f.github, "https://github.com/")), Some("locked"));

    let f = fixture();
    {
        let mut v = f.vault.lock().unwrap();
        let s = v.settings().unwrap();
        v.update_settings(Settings { browser_integration: false, ..s }).unwrap();
    }
    assert_eq!(
        error_code(&open(&f, f.github, "https://github.com/")),
        Some("integration_disabled")
    );
    assert!(f.opened.lock().unwrap().is_empty());
}

#[test]
fn open_item_is_rate_limited_as_a_secret_request() {
    let f = fixture();
    let mut limited = false;
    for _ in 0..20 {
        if error_code(&open(&f, f.github, "https://github.com/")) == Some("rate_limited") {
            limited = true;
            break;
        }
    }
    assert!(limited);
    // It shares the Secret bucket with fill_item.
    let f = fixture();
    for _ in 0..10 {
        open(&f, Uuid::new_v4(), "https://github.com/");
    }
    assert_eq!(error_code(&fill(&f, f.github, "https://github.com/")), Some("rate_limited"));
}

#[test]
fn open_item_without_a_hook_is_an_internal_error() {
    let f = fixture();
    let bare = Bridge::new(f.vault.clone(), || {});
    let r = bare.handle_frame(&request(1, serde_json::json!({
        "type": "open_item", "itemId": f.github, "url": "https://github.com/"
    })));
    let r: serde_json::Value = serde_json::from_slice(&r.to_bytes().unwrap()).unwrap();
    assert_eq!(error_code(&r), Some("internal"));
}
```

(Match the rate-limit loop counts to the existing `secret_requests_are_rate_limited` test if its bucket size differs.)

Run: `cargo test -p havenkeys-bridge open_item` → compile errors (`set_open_item_hook`, `opened`).

- [ ] **Step 2: Implement**

`dispatch.rs` — `enum Dispatched`, new variant:

```rust
    /// The item may be opened in the desktop editor; the caller runs the
    /// hook once the vault lock is released.
    OpenItem(Uuid),
```

(add `use uuid::Uuid;` if not already imported) and in `dispatch`'s match:

```rust
        Request::OpenItem { item_id, url, top_url } => {
            require_enabled(v)?;
            // The same origin binding as fill_item, without decrypting a
            // secret: the item must be one of this page's matches. An unknown
            // ID answers exactly like another site's item.
            let saved_here = v
                .find_matches(url, top_url.as_deref())
                .map_err(code)?
                .iter()
                .any(|s| s.id == *item_id);
            if saved_here {
                Ok(Dispatched::OpenItem(*item_id))
            } else {
                Err(ErrorCode::Denied)
            }
        }
```

`server.rs`:
- `Inner` gets `on_open_item: Mutex<Option<Arc<dyn Fn(Uuid) + Send + Sync>>>,` and `with_writer` initializes it to `Mutex::new(None)`.
- New method on `Bridge`:

```rust
    /// What `open_item` does once the bridge has allowed it: the desktop
    /// shows its window on that item's editor. Called on the bridge's
    /// thread, without the vault lock held. Without a hook, `open_item`
    /// answers `internal`: a bridge with no UI has nothing to open.
    pub fn set_open_item_hook(&self, hook: impl Fn(Uuid) + Send + Sync + 'static) {
        *guard(&self.inner.on_open_item) = Some(Arc::new(hook));
    }
```

- In `handle`, add `| Request::OpenItem { .. }` to the `Some(RequestClass::Secret)` arm, and in the `match dispatched`:

```rust
            Ok(Dispatched::OpenItem(id)) => {
                let hook = guard(&self.inner.on_open_item).clone();
                match hook {
                    Some(hook) => {
                        hook(id);
                        Ok(ResultBody::OpenItem {})
                    }
                    None => Err(ErrorCode::Internal),
                }
            }
```

(add `use uuid::Uuid;` / `std::sync::Arc` imports as needed.)

`ratelimit.rs`: the `Secret` doc comment gains "; `open_item`, which has a visible effect (it raises the desktop window)".
Also update the `dispatch.rs` module doc's list if it enumerates requests.

- [ ] **Step 3: Run**

Run: `cargo test -p havenkeys-bridge && cargo clippy -p havenkeys-bridge --all-targets && cargo fmt --check -p havenkeys-bridge`
Expected: all pass, including the 5 new tests. Also `cargo test -p havenkeys-native-host` (it builds a `Bridge`) → pass.

- [ ] **Step 4: Commit**

```bash
git add crates/havenkeys-bridge
git commit -m "feat(bridge): open_item, origin-bound, calls the desktop hook"
```

---

### Task 3: Desktop — hook, event, editor opening

**Files:**
- Create: `apps/desktop/src/lib/openItem.ts`, `apps/desktop/src/lib/openItem.test.ts`
- Modify: `apps/desktop/src-tauri/src/state.rs` (event constant next to `ITEMS_CHANGED_EVENT`), `apps/desktop/src-tauri/src/lib.rs` (after `Bridge::with_writer(...)` ~line 112-134)
- Modify: `apps/desktop/src/lib/api.ts` (next to `onItemsChanged` ~144)
- Modify: `apps/desktop/src/views/ItemEditor.tsx`, `apps/desktop/src/views/VaultScreen.tsx`
- Modify: `apps/desktop/src/i18n/en.ts`, `apps/desktop/src/i18n/pt-BR.ts` (`vault` block)

**Interfaces:**
- Consumes: `Bridge::set_open_item_hook` (Task 2).
- Produces: event `vault://open-item` with payload `string` (the UUID); `api.onOpenItem(handler: (id: string) => void): Promise<UnlistenFn>`; `decideOpen(pane: PaneRef, dirty: boolean, id: string): "open" | "already" | "confirm"`; `isDirty(initial: EditorSnapshot, current: EditorSnapshot): boolean`; `ItemEditor` prop `onDirtyChange?: (dirty: boolean) => void`.

- [ ] **Step 1: Failing helper tests**

`apps/desktop/src/lib/openItem.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { decideOpen, isDirty, type EditorSnapshot } from "./openItem";

const A = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const B = "16fd2706-8baf-433b-82eb-8c7fada847da";

describe("decideOpen", () => {
  it("opens when nothing is being edited", () => {
    expect(decideOpen({ kind: "empty" }, false, A)).toBe("open");
    expect(decideOpen({ kind: "view", id: B }, false, A)).toBe("open");
    expect(decideOpen({ kind: "view", id: A }, false, A)).toBe("open");
  });
  it("does nothing when that item's editor is already open", () => {
    expect(decideOpen({ kind: "edit", id: A }, false, A)).toBe("already");
    expect(decideOpen({ kind: "edit", id: A }, true, A)).toBe("already");
  });
  it("asks before discarding another edit's changes", () => {
    expect(decideOpen({ kind: "edit", id: B }, true, A)).toBe("confirm");
    expect(decideOpen({ kind: "new" }, true, A)).toBe("confirm");
  });
  it("switches straight away when the other edit is untouched", () => {
    expect(decideOpen({ kind: "edit", id: B }, false, A)).toBe("open");
    expect(decideOpen({ kind: "new" }, false, A)).toBe("open");
  });
});

const base: EditorSnapshot = {
  title: "GitHub",
  username: "alice",
  urls: [{ url: "https://github.com", matchType: "domain" }],
  password: { mode: "keep" },
  totp: { mode: "keep" },
  notes: { mode: "keep" },
  autoSignIn: true,
};

describe("isDirty", () => {
  it("is false for the snapshot it started from", () => {
    expect(isDirty(base, { ...base, urls: [{ ...base.urls[0]! }] })).toBe(false);
  });
  it("is true for any change", () => {
    expect(isDirty(base, { ...base, title: "GitHub 2" })).toBe(true);
    expect(isDirty(base, { ...base, username: "bob" })).toBe(true);
    expect(isDirty(base, { ...base, urls: [] })).toBe(true);
    expect(isDirty(base, { ...base, password: { mode: "set", value: "x" } })).toBe(true);
    expect(isDirty(base, { ...base, totp: { mode: "clear" } })).toBe(true);
    expect(isDirty(base, { ...base, notes: { mode: "set", value: "n" } })).toBe(true);
    expect(isDirty(base, { ...base, autoSignIn: false })).toBe(true);
  });
});
```

Run: `cd apps/desktop && pnpm -s test openItem` → FAIL (cannot resolve `./openItem`).

- [ ] **Step 2: Implement the helpers**

`apps/desktop/src/lib/openItem.ts`:

```ts
import type { SecretEdit } from "./secretEdit";
import type { UrlRule } from "./types";

/** The part of the vault screen's pane that decides what an open request does. */
export type PaneRef = { kind: "empty" } | { kind: "view"; id: string } | { kind: "edit"; id: string } | { kind: "new" };

/**
 * What to do when the browser extension asks to edit `id`:
 * `already` — that editor is open; `confirm` — another edit has unsaved
 * changes, ask before discarding them; `open` — switch to `id`'s editor.
 */
export function decideOpen(pane: PaneRef, dirty: boolean, id: string): "open" | "already" | "confirm" {
  if (pane.kind === "edit" && pane.id === id) return "already";
  if ((pane.kind === "edit" || pane.kind === "new") && dirty) return "confirm";
  return "open";
}

/** Everything the editor would save, for telling an untouched editor from a changed one. */
export interface EditorSnapshot {
  title: string;
  username: string;
  urls: UrlRule[];
  password: SecretEdit;
  totp: SecretEdit;
  notes: SecretEdit;
  autoSignIn: boolean;
}

/** Compared in memory only; never logged or stored. */
export function isDirty(initial: EditorSnapshot, current: EditorSnapshot): boolean {
  return JSON.stringify(initial) !== JSON.stringify(current);
}
```

Run: `pnpm -s test openItem` → 6 tests pass.

- [ ] **Step 3: Rust hook and event**

`src-tauri/src/state.rs`, next to `ITEMS_CHANGED_EVENT`:

```rust
/// The browser extension asked to edit this item (payload: its UUID).
pub const OPEN_ITEM_EVENT: &str = "vault://open-item";
```

`src-tauri/src/lib.rs`, right after the `let bridge = Bridge::with_writer( … );` statement:

```rust
            // "Edit in HavenKeys" from the extension popup. The bridge has
            // already checked the item is saved for the page the popup was
            // opened on; this only raises the window and tells the UI.
            let open_handle = app.handle().clone();
            bridge.set_open_item_hook(move |id| {
                tray::show_main_window(&open_handle);
                let _ = open_handle.emit(state::OPEN_ITEM_EVENT, id.to_string());
            });
```

Run: `cd apps/desktop/src-tauri && cargo clippy -q --all-targets && cargo test -q` → clean / pass.

- [ ] **Step 4: API and strings**

`src/lib/api.ts`, after `onItemsChanged`:

```ts
  /** The browser extension asked to edit this item (already checked against the page in Rust). */
  onOpenItem: (handler: (id: string) => void): Promise<UnlistenFn> =>
    listen<string>("vault://open-item", (e) => handler(e.payload)),
```

`en.ts` `vault` block:

```ts
    discardChanges: (title: string) => `Discard your changes to “${title}”?`,
    discardNewItem: "Discard the new item?",
    keepEditing: "Keep editing",
    discard: "Discard",
```

`pt-BR.ts` `vault` block:

```ts
    discardChanges: (title: string) => `Descartar suas alterações em “${title}”?`,
    discardNewItem: "Descartar o novo item?",
    keepEditing: "Continuar editando",
    discard: "Descartar",
```

- [ ] **Step 5: ItemEditor reports unsaved changes**

In `ItemEditor.tsx`: add `onDirtyChange?: (dirty: boolean) => void;` to `Props` and destructure it. Import `isDirty, type EditorSnapshot` from `../lib/openItem`. After the existing `useState` declarations:

```ts
  const snapshot: EditorSnapshot = { title, username, urls, password, totp, notes, autoSignIn };
  const [initialSnapshot] = useState(snapshot);
  const dirty = isDirty(initialSnapshot, snapshot);
  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);
  useEffect(() => () => onDirtyChange?.(false), [onDirtyChange]);
```

(The second effect reports "clean" when the editor unmounts, e.g. after Save or Cancel.)

- [ ] **Step 6: VaultScreen listens and confirms**

In `VaultScreen.tsx`:

```ts
import { decideOpen } from "../lib/openItem";
```

State, next to `pane`:

```ts
  const [editorDirty, setEditorDirty] = useState(false);
  const [pendingOpen, setPendingOpen] = useState<string | null>(null);
```

Helpers and listener, after the `onItemsChanged` effect:

```ts
  const openForEdit = useCallback((id: string) => {
    setPendingOpen(null);
    setSection("login");
    setQuery("");
    setPane({ kind: "edit", id });
  }, []);

  // "Edit in HavenKeys" from the browser extension's popup.
  useEffect(() => {
    const unlisten = api.onOpenItem((id) => {
      const ref = pane.kind === "new" ? { kind: "new" as const } : pane;
      switch (decideOpen(ref, editorDirty, id)) {
        case "already":
          return;
        case "confirm":
          setPendingOpen(id);
          return;
        case "open":
          openForEdit(id);
      }
    });
    return () => void unlisten.then((f) => f());
  }, [pane, editorDirty, openForEdit]);
```

Pass `onDirtyChange={setEditorDirty}` to **both** `<ItemEditor … />` elements (edit and new). Above the pane's content (first child of the `<section>` that holds the panes, before `{pane.kind === "empty" && …}`):

```tsx
            {pendingOpen && (
              <div className="confirm open-confirm" role="alert">
                <span>
                  {pane.kind === "edit" && selected ? t.vault.discardChanges(selected.title) : t.vault.discardNewItem}
                </span>
                <button className="btn btn-small" onClick={() => setPendingOpen(null)}>
                  {t.vault.keepEditing}
                </button>
                <button className="btn btn-small btn-danger" onClick={() => openForEdit(pendingOpen)}>
                  {t.vault.discard}
                </button>
              </div>
            )}
```

`src/styles.css`, after the `.confirm` rule:

```css
.open-confirm {
  margin: 0 0 14px;
  padding: 10px 14px;
  border: 1px solid var(--brass);
  border-radius: 12px;
  background: var(--sel);
}
```

- [ ] **Step 7: Verify**

Run: `cd apps/desktop && pnpm -s typecheck && pnpm -s test`
Expected: typecheck clean; all tests pass (+6 new).

- [ ] **Step 8: Commit**

```bash
git add apps/desktop
git commit -m "feat(desktop): open a login's editor when the extension asks"
```

---

### Task 4: Extension — popup button and handler

**Files:**
- Modify: `apps/extension/src/messaging/popup.ts` (`PopupRequest`, `parsePopupRequest`)
- Modify: `apps/extension/src/background/popup-handler.ts` (`handle`)
- Modify: `apps/extension/src/popup/popup.ts` (`matchRow`), `apps/extension/src/popup/popup.css`
- Modify: `apps/extension/src/i18n/en.ts`, `apps/extension/src/i18n/pt-BR.ts` (`popup` block)
- Test: `apps/extension/src/background/popup-handler.test.ts`

**Interfaces:**
- Consumes: protocol request `{ type: "open_item"; itemId; url }` and result `{ type: "open_item" }` (Task 1).
- Produces: popup message `{ type: "popup_open_item"; itemId: string }` → `PopupReply<null>`.

- [ ] **Step 1: Failing tests**

In `popup-handler.test.ts`, in "accepts the popup requests":
`expect(parsePopupRequest({ type: "popup_open_item", itemId: ID })).toEqual({ type: "popup_open_item", itemId: ID });`
In "rejects everything else" list add:
`{ type: "popup_open_item", itemId: ID, url: "https://github.com" },`
`{ type: "popup_open_item", itemId: "x" },`
`{ type: "open_item", itemId: ID, url: "https://github.com" },`
In `describe("popup handler")` add:

```ts
  it("asks the desktop to open the login, using the tab's URL", async () => {
    const c = fakeClient(() => ({ type: "open_item" }));
    const h = createPopupHandler(c, async () => ({ id: 1, url: "https://github.com/login?next=x#y" }));
    expect(await h.handle({ type: "popup_open_item", itemId: ID })).toEqual({ ok: true, value: null });
    expect(c.seen).toEqual([{ type: "open_item", itemId: ID, url: "https://github.com/login" }]);
  });

  it("refuses to open on non-web pages without contacting the host", async () => {
    const c = fakeClient(() => ({}));
    const h = createPopupHandler(c, async () => ({ id: 1, url: "chrome://newtab/" }));
    expect((await h.handle({ type: "popup_open_item", itemId: ID })).ok).toBe(false);
    expect(c.seen).toHaveLength(0);
  });

  it("passes the desktop's refusal to the popup", async () => {
    const c = fakeClient(() => {
      throw new BridgeError("denied", "This item is not saved for this website.");
    });
    const h = createPopupHandler(c, async () => ({ id: 1, url: "https://evil.com/" }));
    expect(await h.handle({ type: "popup_open_item", itemId: ID })).toEqual({
      ok: false,
      message: "This item is not saved for this website.",
    });
  });
```

Run: `cd apps/extension && pnpm -s test popup-handler` → FAIL.

- [ ] **Step 2: Message type and handler**

`messaging/popup.ts`: add to `PopupRequest`:

```ts
  /** Show this login in the desktop app's editor. */
  | { type: "popup_open_item"; itemId: string };
```

and add `case "popup_open_item":` to the `popup_totp` / `popup_fill` / `popup_fill_totp` group in `parsePopupRequest`.

`background/popup-handler.ts`, new case in `handle`:

```ts
      case "popup_open_item": {
        // Same rule as fill: the URL is the tab's, and the desktop opens
        // only a login saved for it.
        const url = pageUrlForRequest(await activeTabUrl());
        if (!url) return { ok: false, message: t.errors.pageNotSupported };
        try {
          await client.request({ type: "open_item", itemId: req.itemId, url });
          return { ok: true, value: null };
        } catch (e) {
          return fail(e);
        }
      }
```

Run: `pnpm -s test popup-handler` → pass.

- [ ] **Step 3: Popup button**

`i18n/en.ts` `popup` block: `edit: "Edit in HavenKeys",`. `i18n/pt-BR.ts` `popup` block: `edit: "Editar no HavenKeys",`.

`popup/popup.ts`, a helper next to `smallButton`:

```ts
const SVG = "http://www.w3.org/2000/svg";

/** A square icon button; the icon is built with createElementNS, never parsed from markup. */
function iconButton(pathData: string, label: string): HTMLButtonElement {
  const b = h("button", { className: "icon-btn row-icon" });
  b.type = "button";
  b.title = label;
  b.setAttribute("aria-label", label);
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(SVG, "path");
  path.setAttribute("d", pathData);
  svg.append(path);
  b.append(svg);
  return b;
}

const PENCIL = "M4.5 19.5h4l10-10a2.1 2.1 0 0 0-3-3l-10 10v3zM14 8l3 3";
```

In `matchRow`, immediately before `// The status line (a fill or code error)…` / `row.append(actions, status);`:

```ts
  // Last in the row: open this login in the desktop app. Closes the popup
  // on success, like a fill, since focus moves to the app.
  const edit = iconButton(PENCIL, t.popup.edit);
  edit.addEventListener("click", () => void fillFromPopup(edit, { type: "popup_open_item", itemId: m.id }, status));
  actions.append(edit);
```

(`fillFromPopup` already disables the button, sends the request, closes the popup on success and writes the error into `status` otherwise.)

`popup/popup.css`, after the `.actions .btn.small…` rules:

```css
.actions .row-icon {
  width: 28px;
  height: 28px;
}
```

(Keep the existing `.icon-btn` look; adjust the size to match `.btn.small`'s height if it differs, and say so in the report.)

- [ ] **Step 4: Verify**

Run: `cd apps/extension && pnpm -s test && pnpm -s typecheck` (or the package's type-check script)
Expected: all pass, including the hygiene test that forbids `innerHTML` and the i18n parity tests.

- [ ] **Step 5: Commit**

```bash
git add apps/extension
git commit -m "feat(extension): edit a login in the desktop app from the popup"
```

---

### Task 5: Security docs, spec touch-ups, full verification

**Files:**
- Modify: `docs/security-model.md:247` (bridge request list), `docs/threat-model.md` (compromised-extension section, near line 381)
- Modify: `docs/superpowers/specs/2026-09-27-popup-open-in-desktop-design.md` (§4.1 result name, §7 popup test)

- [ ] **Step 1: Security model**

In the sentence listing the bridge's request set (`status`, `lock`, `find_matches`, `fill_item`, `get_totp`, …), add `open_item` after `get_totp`, and add after that paragraph:

```markdown
`open_item` returns nothing: when the item is a login saved for the page,
the desktop shows its window with that login's editor open. It is in the
`secret` rate class because it has a visible effect.
```

- [ ] **Step 2: Threat model**

In the compromised-extension part (search "compromised extension"), add:

```markdown
* **Raising the desktop window.** A compromised extension can send
  `open_item` for a login saved for the page it reports, which brings the
  desktop window forward on that login's editor. It receives nothing, the
  editor saves nothing without the user, an unsaved edit elsewhere is
  protected by a discard prompt, and the request shares the `secret` rate
  limit.
```

- [ ] **Step 3: Spec touch-ups**

Spec §4.1: the result is `ResultBody::OpenItem {}`, wire `{"type":"open_item"}` (every result names its request). Spec §7 Extension: the popup's DOM has no test harness; the button's placement is checked manually, and `popup_open_item` parsing and handling are unit-tested.

- [ ] **Step 4: Full verification**

```bash
cargo fmt --check
cargo clippy -q --all-targets -p havenkeys-protocol -p havenkeys-bridge -p havenkeys-native-host
cargo test -q -p havenkeys-protocol -p havenkeys-bridge -p havenkeys-native-host
(cd apps/desktop/src-tauri && cargo fmt --check && cargo clippy -q --all-targets && cargo test -q)
pnpm -r test
(cd apps/desktop && pnpm -s typecheck)
cargo deny check
```

Expected: all clean / pass.

- [ ] **Step 5: Commit**

```bash
git add docs
git commit -m "docs: open_item in the security and threat models"
```

- [ ] **Step 6: Manual check (the user)**

With the desktop app running and unlocked and the extension loaded: open github.com (with a saved GitHub login) → popup → pencil on the GitHub row → the desktop window comes forward on GitHub's editor and the popup closes. Repeat with the window hidden in the tray; repeat while another login's editor has unsaved changes (expect the discard prompt).
