# The HavenKeys Account item — Design

Status: accepted, 2026-09-29.

> This software has not undergone an independent security audit.

## 1. Goal

1Password puts a "1Password Account" item at the start of every vault: the
address, the Secret Key and the sign-in site, one search away. HavenKeys keeps
the same facts in Settings → Account and the Emergency Kit, which is hard to
find while setting up a second computer.

Every unlocked vault shows a **HavenKeys Account** item pinned first in
All items. It shows the account's email, server, account ID and Secret Key,
so the user can read or copy them without going into Settings.

## 2. Decisions taken

| Question | Decision | Rejected alternatives |
|---|---|---|
| Where the item lives | Virtual: built from the local account record and the keychain when shown; never stored | A stored `account` item type written at activation and synced |
| Master password | Not shown or stored. The item has no password field | An optional field the user types; a field Rust fills at activation and on password change |
| Editable | No: no Edit, no Delete, no notes | Editable like any login |
| Where it shows | Desktop app only | Extension popup / autofill |
| Secret Key to the renderer | Only on an explicit reveal; copy goes Rust → clipboard | Sent with the item on selection |

Why virtual. A stored item would put a second copy of the Secret Key into
vault ciphertext on the server. That does not weaken the crypto (opening it
needs the Secret Key already), but it is a new copy to reason about. A stored
item would also need a new item type across core, protocol, sync and UI, a
"create if missing" step for vaults activated before this change that avoids
duplicates across devices, and handling for edits that make it disagree with
the real account. A virtual item follows the account as it is, cannot go
stale and cannot be deleted.

Why no master password. CLAUDE.md §9 says the master password is never
persisted. Encrypting it inside the vault it unlocks is circular and adds
nothing for recovery, and it would go stale on a password change.

## 3. What does not change

* The vault format, the item model (`ItemType` stays `Login | SecureNote`),
  sync, the server, and the protocol crate.
* Native messaging. The item is not in the item store, so `find_matches`,
  `fill_item`, `get_totp` and the passkey commands cannot see it. The
  extension's view of the vault is unchanged by construction.
* The Emergency Kit and Settings → Account, which stay where they are.

## 4. Rust (desktop, `src-tauri/src/account.rs`)

The non-secret facts already reach the UI through `account_status` (email,
server URL, last sync time). Two commands are added next to
`get_emergency_kit`, and use the same checks as it does:

```text
reveal_account_secret_key()                  -> SecretString
copy_account_field(field: AccountCopyField)  -> CopyResult { clearAfterSeconds }

AccountCopyField = "email" | "server" | "account_id" | "secret_key"
```

Both:

1. call `state.touch()` (the reveal counts as activity for auto-lock);
2. refuse with `Locked` unless the vault is unlocked;
3. read the account from the vault (`NoVault` if none) and, when the Secret
   Key is needed, its text from the device store (`NotFound` if this
   computer does not hold it);
4. never log the key, and return errors that do not contain it.

`copy_account_field` reads the value in Rust and hands it to
`state.clipboard.copy` with the vault's `clipboard_clear_seconds`, as
`copy_secret` does for every field (the renderer has no clipboard access of
its own). The Secret Key does not pass through the renderer on a copy. The
field is an enum; an unknown value fails deserialization.

`account_status` already returns `email`, `serverUrl` and `accountId`, so the
detail view needs no new command and no Secret Key to render.

Both commands are added to the command allowlist in `lib.rs`; no capability
file change is needed beyond what registering a command requires.

## 5. Desktop UI

### 5.1 List

`ItemList` gets an optional pinned row above the items, rendered only in the
**All items** section and only when `account_status` returned an account:

* the seal (`Seal.tsx`) as its icon, title "HavenKeys Account", the email as
  its subtitle;
* included in search when the query matches "havenkeys", the email, or the
  server host (case-insensitive substring, the same rule the list uses);
* not counted in the All / Logins / Secure notes counts;
* selected state and keyboard navigation behave like any other row.

### 5.2 Detail

`VaultScreen`'s `Pane` gains `{ kind: "account" }`. Selecting the pinned row
sets it, and it renders a new `AccountItemDetail` view, laid out like
`ItemDetail` (header, fields with copy buttons, sections):

```text
[seal]  HavenKeys Account
        me@example.com

email       me@example.com                        [copy]
server      https://vault.example.com             [copy]

SECRET KEY
secret key  ••••••••••                     [eye]  [copy]

account ID  3f1c…                                 [copy]

Use these with your master password to sign in to HavenKeys on
another computer.                          [Show Emergency Kit]
```

* The eye calls `reveal_account_secret_key`. The revealed key lives only in
  that component's state and is dropped when the component unmounts, which
  includes lock (as `EmergencyKit.tsx` does). Selecting another item hides it
  again.
* Every copy button calls `copy_account_field` with its field and shows the
  usual "clears in N seconds" toast, through `CopyButton`.
* When this computer lacks the Secret Key, the section shows "The Secret Key
  is not on this computer" and no eye/copy buttons.
* **Show Emergency Kit** switches to Settings and scrolls to the Account
  block's kit.
* No Edit, Delete, favourite or "open website" actions.
* The item and its fields are never put in the window title, a URL or a
  notification.

### 5.3 Strings

New strings under `accountItem` in `i18n/en.ts` and `i18n/pt-BR.ts`: title,
note, "not on this computer", and labels. Labels already present in `common`
(email, server, Secret Key) are reused.

## 6. Error handling

| Case | Behaviour |
|---|---|
| Vault locked during a reveal | `Locked`; the app is already moving to the unlock screen |
| No account record | No pinned row (cannot happen after activation, but handled) |
| Secret Key missing from keychain | Row and detail shown; key section says it is not here |
| Clipboard failure | The existing clipboard error toast |

## 7. Testing

Rust (`account.rs` tests):

* both commands return `Locked` while locked (every copy field);
* reveal and `copy_account_field(secret_key)` return `NotFound` when the
  device store has no key for the account; the other copy fields still work;
* reveal returns exactly the stored key text when unlocked;
* the key text does not appear in the `Debug` or error output of anything
  returned.

Frontend (Vitest, pure functions pulled out for it):

* the search predicate matches "havenkeys", the email and the server host,
  and does not match unrelated text;
* the pinned row appears only in All items and only with an account.

Then the usual phase checks: `cargo test`, `cargo clippy`, `tsc`, the desktop
test suite, and a manual check with `tools/ui-check` that the row, reveal,
copy, lock-hides-key and missing-key states render in both languages.

## 8. Documentation

* `docs/security-model.md`: one paragraph listing the two commands, and
  noting that the item is virtual and never reaches the extension or the
  server.
* `README.md` feature list: one line.
