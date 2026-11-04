# Bank Account item and agência/conta fill — Design

Status: draft, 2026-11-03. Roadmap item B6 (`docs/autofill-roadmap.md`).
Builds on the card item (`2026-09-29-card-item-design.md`), the shared field
words (`2026-10-19-shared-field-words-design.md`) and the Android direct fill
(`2026-10-01-android-app-design.md` §7.6). Amends CLAUDE.md (a note, §7.4).

> This software has not undergone an independent security audit.

## 1. Goal

Brazilian banks sign in to internet banking with **agência + conta** (the
branch and account numbers) instead of a username. Some ask for the account's
check digit (*dígito*) in a box of its own (Bradesco). Others want it typed
after a hyphen in the account box (Itaú `12345-6`, Banco do Brasil's agência
`1234-5`). The password comes next, often on a second page. Today HavenKeys
calls the agência or conta box a username (five known gaps in
`packages/field-words/fixtures/br-fields.json`) and has nothing to put in it.

A new **Bank Account** item holds those numbers with the bank's websites and
internet-banking password. On a bank's sign-in page, one pick in the
HavenKeys menu fills agência, conta and dígito in whatever layout the page
uses, plus the password when it is on the same page. On the next page, the
password box offers the same item.

Success: on the five banks in the fixture (Itaú, Bradesco, Banco do Brasil,
Santander, Sicoob), the boxes are classified as bank fields on both the
extension and Android, the B6 known gaps are removed from the fixture, and a
pick fills them.

## 2. Non-goals and anti-goals

**Not in this spec:**

* **Virtual keyboards.** Itaú's and Bradesco's on-screen password keypads
  can't be filled, and HavenKeys won't try.
* **Bank tokens** (iToken, chave de segurança). They are not TOTP.
* **Filling bank details into payment forms on other sites** (direct-debit
  donation forms like TETO's). A bank account fills only on its own saved
  websites. Those forms' fields are classified as bank fields, so they no
  longer get the login menu, but no bank account is offered there.
* **IBAN, SWIFT, US routing numbers and PIX keys** as structured fields.
  They go in custom fields or notes. Imports put them there (§6).
* **A "save this bank account?" prompt** after a typed sign-in. The user
  creates the item in the app.
* **Bank logos.** A generic bank glyph is used. Licences for bank marks are
  not as clear as for card networks.
* **Vault health checks for bank passwords.** Banks impose 6- or 8-digit
  passwords the user can't strengthen, so health checks skip bank accounts.

**Anti-goals** (would make this a failure even if it works):

* A bank account filled on any site it is not saved for, or in a frame whose
  page is not also saved for it. The binding is the login's (CLAUDE.md §32).
* A bank sign-in submitted or continued without the user's pick. **No auto
  sign-in:** no button press and no automatic password step, whatever the
  vault's auto sign-in setting says.
* "Conta" taken as a bank field on ordinary sites ("Entrar na sua conta",
  "Criar conta"). That would break login fill on many Portuguese sites.

## 3. Decisions taken

These are my calls. The owner skipped the questions.

| Question | Decision | Rejected alternatives |
|---|---|---|
| Item or login fields | A new item type, `bank_account`, that is **site-bound like a login** (websites, Android app bindings, password) | Bank fields added to Login (cheaper, but splits one item kind across two meanings); an unbound item like Card |
| Fill request | The existing `fill_item` / `autofill_fill` path, whose reply gains the bank values | New `find_bank` / `fill_bank` requests (a second origin-binding path to audit) |
| Where bank items are offered | The **bank menu** on agência/conta/dígito boxes (bank accounts, then the site's logins); the **login menu on password boxes**, after the site's logins | Every login menu (a bank item has nothing for a username box); a bank menu without logins (existing bank logins would stop filling, owner's decision) |
| What one pick fills | Every bank box of that form, plus its password box if there is one | Bank boxes only (then the password needs a second pick on the same page) |
| Check digit layout | Rust returns the digits separately. The extension and Android join them to fit each box (§5.2) | Rust formats per box (it would need box shapes in the request) |
| Auto sign-in | Never for bank accounts | Following the vault setting |
| Overview contents | Bank name, agência, the account's last 3 characters | The full account number (it would appear in plain lists and search) |

## 4. Data model (`havenkeys-core`)

### 4.1 Type

`ItemType` gains `BankAccount` (`"bank_account"` on the wire). `ItemDetails`
gains `BankAccount(Box<BankAccountFields>)`. Fields, validation and
formatting go in a new `src/bank.rs`, as cards do in `src/card.rs`.

### 4.2 Fields

The field names below are camelCase on the wire, with
`deny_unknown_fields`. Items also get the usual generic parts: title, tags,
attachments, created/updated.

| Field | Type | Rule |
|---|---|---|
| `bank` | `Option<String>` | ≤ 64 chars, no control characters ("Itaú", "Banco do Brasil") |
| `holderName` | `Option<SecretString>` | ≤ 256 chars, no control characters |
| `accountType` | `Option<BankAccountType>` | `checking`, `savings`, `payment`, `other` |
| `branch` | `Option<String>` | 1–8 ASCII digits |
| `branchDigit` | `Option<String>` | one char, `0`–`9` or `X` (BB uses X) |
| `account` | `Option<SecretString>` | 1–20 ASCII digits |
| `accountDigit` | `Option<SecretString>` | 1–2 chars, `0`–`9` or `X` |
| `password` | `Option<SecretString>` | as a login password (`MAX_PASSWORD_CHARS`) |
| `notes` | `Option<SecretString>` | as login notes |
| `sections` | `Vec<FieldSection>` | custom fields, as logins and notes |
| `appBindings` | `Vec<AppBinding>` | as logins; carried over by every edit |

Website rules (`urls`, `matchType`) live in the overview, exactly as a
login's do, with the same limits.

Input cleanup in Rust, so the editor, imports and paste behave the same:
* spaces and `.` are removed;
* a trailing `-D` / `-DD` on `branch` or `account` is split into its digit
  field when that field is empty;
* a lowercase `x` becomes `X`;
* whitespace-only values are stored as absent.

A bank account needs a title, a bank or an account, so an empty one is
rejected. `Debug` of the fields prints no values.

### 4.3 Overview and search

`ItemOverview` gains:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub bank: Option<BankSummary>,   // { bank, branch, accountLast3 }
```

* `title`: the user's title, else `bank`, else "Bank account".
* `accountLast3`: the last three characters of the account and digit taken
  together, with a `-` before the digit part: `12345` / `6` gives `45-6`,
  `98765` with no digit gives `765`, `12345` / `66` gives `5-66`. It is absent
  when the account is shorter than 4 digits.
* `username`: none.

Search matches title, bank and branch. It never matches the full account
number.

### 4.4 Site binding

Every place that binds a login to a page or app also accepts a bank account:
* page matching;
* `authorize_for_page` and `fill_for_page`;
* `matches_for_app` and `fill_for_app`;
* binding an app from search;
* lookalike checks;
* frecency recording, when anything was filled.

The way to do this is the builder's call; a helper such as
`ItemType::is_site_bound()` is one option. These stay login-only:
* TOTP (a bank account has none);
* passkeys and "Sign in with";
* password history;
* `auto_sign_in_for`, which returns `false` for a bank account;
* save checks.

There is one exception for save checks. `check_login` counts a bank account
saved for the page whose password equals the typed one as **Unchanged**.
Without it, the second page's password would prompt to save a duplicate
login.

`Suggestion` (find_matches and Android matches) gains
`kind: "login" | "bank_account"` and, for a bank account, its `BankSummary`
instead of a username.

### 4.5 Fill reply

`FillCredentials` gains `bank: Option<BankValues>`, which is filled only for
a bank account:

```text
BankValues { branch, branchDigit?, account, accountDigit? }
```

A bank account's reply has `username: None`, its password and its bank
values. A login's reply is unchanged. Nothing else of the item leaves Rust:
not the holder name, notes or custom fields.

### 4.6 Input and edits

`ItemInput` gains `bank: Option<BankAccountInput>`. In it, `account` and
`accountDigit` are `SecretUpdate` (Keep/Set/Clear), as a card's number is.
The password, notes, websites and custom fields use `ItemInput`'s existing
top-level fields, as a login's do. `check_shape` requires the bank input for this type and
rejects it for every other type. A change of type into or out of
`BankAccount` is refused. Import dedupe treats two bank accounts as the same
when bank, branch and account are equal.

### 4.7 Compatibility

The server stores blobs without knowing their type: nothing changes there.
An older desktop or Android app that pulls a bank account records it as
unreadable (`sync.rs` `unreadable_items`) and retries after an update. Its
sync doesn't fail. That's acceptable, and the release notes say to update
every device. The encrypted backup carries bank accounts with no change.

## 5. Autofill

### 5.1 Classification (shared words, extension and Android)

`packages/field-words/words.json` gets new lists, which are regenerated
into `words.generated.ts` and `SharedWords.kt` as B5 set up:

* `BANK_BRANCH`: `agencia`, `ag`, `cod agencia`, `numero da agencia`,
  `branch`, `cooperativa`, `dependencia`.
* `BANK_ACCOUNT`: `conta`, `conta corrente`, `numero da conta`,
  `conta poupanca`, `account number`, `bank account`.
* `BANK_DIGIT`: `digito`, `dv`, `dig`, `digito da conta`,
  `digito da agencia`.
* `BANK_NEGATIVE`: `criar conta`, `minha conta`, `sua conta`, `nova conta`,
  `entrar na conta`, `email`, `e mail`, `senha`, `usuario`, `cpf`, `cnpj`.

Roles: `bankBranch`, `bankBranchDigit`, `bankAccount`, `bankAccountDigit`.
A box gets a bank role under these rules:

1. **Branch**: a `BANK_BRANCH` word, and either the box is numeric-looking
   (`type=number|tel`, `inputmode=numeric`, or `maxlength` ≤ 8) **or** the
   same group has a box with a `BANK_ACCOUNT` word. Sicoob's lone
   "cooperativa" box qualifies by shape. Santander's and TETO's text
   "Ag"/"Agência" boxes qualify by their conta neighbour. `BANK_NEGATIVE`
   also holds `agencia de viagem`, `agencia de turismo`, `travel agency`,
   `agency name`.
2. **Account**: a `BANK_ACCOUNT` word and no `BANK_NEGATIVE` phrase, **and**
   the same group has a branch box. **Conta never counts on its own.**
3. **Digit**: a `BANK_DIGIT` word or `maxlength` 1–2, placed right after a
   branch or account box. It takes that box's digit role.
4. Never a bank field: password boxes, one-time-code boxes, hidden, disabled
   or read-only boxes, and boxes with a card or identity role.

The login classifier refuses a box that has a bank role, so agência never
becomes a username again. Itaú's "Código do operador" and CPF boxes stay
usernames.

Fixture (my call, from the builder check). Rules 1–3 depend on neighbouring
boxes, so the shared fixture format grows, in both harnesses
(`br-fields.test.ts`, `BrFieldsTest.kt`):

* each record gains a fourth expectation, `bank`: a role, `"none"` or
  `"skip"`;
* `context` gains the group facts the rules read: `groupHasBranch`,
  `groupHasAccount`, and `prevRole` (the bank role of the box just before
  it, or `null`).

Both harnesses pass these to the classifier as they pass `hasPassword`
today. Every existing record gets `bank: "none"` except:
* the bank boxes of Itaú, Bradesco, Banco do Brasil, Santander and Sicoob;
* TETO's agência and conta boxes.

Those get their roles and group facts, and the 10 `known_gaps` with reason
"B6" are removed. TETO's account box classifies as `bankAccount`
(that page has an agência box), which ends its login gap. Like any other
site that has no saved bank account, it is offered nothing.

### 5.2 Writing the values

The extension (`autofill/bank.ts`, new) and Android shape each value to its
box:

* If the form has a separate digit box, the number box gets the number alone
  and the digit box gets the digit.
* Otherwise, when the item has a digit:
  * `NNNN-D` when the box accepts a hyphen: not `type=number`, and no
    `maxlength` or `maxlength` ≥ len + 2;
  * else `NNNND` when the box takes len + 1;
  * else the number alone.

  This covers Itaú's account box (`maxlength` 7) and BB's branch box
  (`maxlength` 6).
* Input masks are handled by the existing fill path (events per character as
  for card numbers). The builder checks Itaú and BB in a real browser.

Fill rules are the login fill's:
* only visible, editable boxes of the picked form, re-checked when writing;
* a frame only if it and its top page are both saved for the item;
* existing values are never overwritten unless HavenKeys wrote them.

**A bank fill never submits.**

### 5.3 Extension menu and popup

* New `MenuKind` `"bank"` on a bank box, on the same trusted events and
  preference as the login menu. Rows come from `find_matches` filtered to
  `kind: bank_account`. Each row shows a bank glyph, the title and
  `Ag 1234 · Conta •••45-6`.
* **The site's logins are listed too, after its bank accounts** (owner's
  decision, 2026-11-03). Today the only way to fill these banks is a login
  whose username holds the agência or conta. Picking such a login fills its
  username into the box the menu opened on, and its password into the form,
  as the login menu does today. So no saved login stops filling when B6
  ships, and no conversion is needed.
* No bank account and no login for the site: the menu does not open on focus,
  like the empty login menu (it would intrude on donation and checkout forms
  with an agência box). It opens only from the field icon, and then shows
  "No bank account saved for this site — add one in HavenKeys". Locked: the
  usual unlock row.
* On a password box, the login menu lists the site's logins, then its bank
  accounts. Picking one fills that form's password, and its bank boxes if
  it has any.
* The popup lists the site's bank accounts after its logins with Fill, as
  a login's row. The popup has no password copy, as for logins. While the
  account is frozen there is no Fill and no copy.
* `fill_item` needs no protocol change beyond the reply's `bank` field and
  the suggestion's `kind`/`bank`. The native host's size limits and
  validation apply as today.

### 5.4 Android

* The Kotlin classifier gets the same roles and rules as §5.1, from
  `SharedWords.kt`.
* A target with bank roles still gets the site's or app's login datasets,
  after its bank-account ones, as in §5.3. A login's username goes into the
  focused bank box.
* A target with bank roles or a password gets one dataset per matched bank
  account. The dataset carries every bank box and the password, shaped as
  in §5.2, and is labelled with the title and `Ag 1234 · •••45-6`.
* **Direct fill:** while unlocked, a matched bank account's values may go to
  the autofill framework, as a matched login's do. This needs the CLAUDE.md
  note in §7.4. "Confirm before filling" and the rows offered after an
  unlock open HavenKeys first, as for logins.
* Bank apps match through app bindings and verified asset links, as logins
  do. Many bank apps turn off autofill; that is not a bug of ours.

## 6. Apps

### 6.1 Desktop

* Rust commands, following the card's pattern:
  * `reveal_bank(id)` returns bank, holder, type, branch, branch digit,
    notes, sections, and `hasAccount` / `hasPassword` flags;
  * `reveal_bank_field(id, field)` with `account`, `accountDigit` or
    `password`;
  * `copy_bank_field(id, field)` with `bank`, `holder`, `branch` (with its
    digit, `1234-5`), `account` (`12345-6`), `password` or `details`
    (a text Rust composes from the non-secret fields and the account), using
    the clipboard-clear delay. The account type has no copy;
  * each command is added in the four places `commands.test.ts` checks.
* Sidebar: a **Bank accounts** entry with a count, after Cards. **New → Bank
  account**.
* List row: the glyph, the title and `Ag 1234 · Conta •••45-6`.
* Detail, showing the fields that have values, in this order (the editor
  uses the same order):
  * bank;
  * agência `1234-5`;
  * conta `•••45-6`, revealed by the eye;
  * account type;
  * holder;
  * password, labelled "Password" (pt-BR "Senha"), masked;
  * websites, as on a login;
  * notes;
  * custom fields.

  Copy buttons sit on bank, holder, agência, conta and password (not on the
  type), plus a **Copy details** action. Reveals hide after 30 s and on lock.
* Editor:
  * title;
  * bank;
  * agência + dígito, two boxes side by side;
  * conta + dígito, two boxes side by side;
  * account type;
  * holder;
  * password (keep/replace, with the generator, as a login's);
  * websites (the login's website editor);
  * notes;
  * custom fields.

  Pasting `12345-6` into a number box splits it into the two boxes; typing never splits a number, only a paste does.
* Strings in `en` and `pt-BR`. pt-BR: Banco, Titular, Tipo de conta
  (Corrente, Poupança, Pagamento, Outra), Agência, Conta, Dígito, Senha.
  The English list subtitle says "Branch 1234".

### 6.2 Android app

* The item in the list, filter, detail (reveal and copy) and editor, with
  the same fields and rules as the desktop. Phone layout follows the card's
  screens, and phone conventions win where they differ: the editor keeps the
  saved account behind Change / Keep / Remove, and the detail keeps notes
  behind an eye, as phone logins and cards do.
* Autofill: a bank dataset overwrites existing box values, as Android login
  fill does (the framework replaces values).
* Activity record: copying or filling a bank account counts as for a login.

### 6.3 Import and export

* **1Password (1PUX)**, category `101` (Bank Account):
  * `bankName` → bank;
  * `owner` → holder;
  * `accountType` → type;
  * `accountNo` → account, with a trailing `-D` split off;
  * `branch*` / `routingNo` → branch only when they are 1–8 digits.

  Everything else goes to notes as `label: value` lines: routing, SWIFT,
  IBAN, PIN, phone and address. The same applies to a value that fails
  validation.
* **Bitwarden JSON import**: Bitwarden has no bank type; nothing changes.
* **Plaintext exports**:
  * Bitwarden JSON writes a bank account as a login with its URIs and
    password, and custom fields for bank, agência and conta (so another
    manager can still fill the password);
  * CSV writes the same columns as a login, plus the bank fields in notes;
  * both follow §38.

## 7. Security

### 7.1 Trust

The binding is the login's, enforced in Rust. The extension's item id,
origin and box roles are untrusted. A bank account is denied in these
cases:
* the vault is locked;
* the item is not a site-bound type;
* the page (and top page) doesn't match the item's website rules;
* on Android, the app doesn't match its bindings or verified hosts.

### 7.2 What leaves Rust

* `find_matches` returns only the summary: bank, agência and the account's
  last 3 characters.
* `fill_item` returns the bank values and the password for the picked item,
  and nothing more.
* Agência is in the overview. A branch number is printed on cheques and
  isn't secret. The full account is kept in the encrypted details.

### 7.3 Docs

* `docs/security-model.md`: the item, what is in its overview, its binding,
  that it never auto-submits.
* `docs/autofill.md`: the bank roles and the conta-needs-agência rule.
* `docs/threat-model.md`: a phishing page that copies a bank's agência/conta
  form gets no bank menu rows, because it is not the saved site. The
  lookalike warning applies.
* `README.md`: one line in the feature list.

### 7.4 CLAUDE.md note

Add under the Android amendments:

> Amended on 2026-11-03 by `docs/superpowers/specs/2026-11-03-bank-account-design.md`:
> a Bank Account item (agência, conta, dígito, password) is bound to its
> websites and apps like a login. Android direct fill covers it as it covers
> logins. A bank fill never submits and never continues a sign-in on its own.

## 8. Testing

Core (`crates/havenkeys-core/tests/bank.rs`):
* the item round-trips through seal/unseal;
* the overview holds only bank, branch and last 3;
* validation: each limit, `-D` splitting, `x` → `X`, empty item rejected,
  unknown fields rejected;
* `SecretUpdate` keep/set/clear for account, digit and password;
* search by bank and branch, never by the full account;
* `Debug` shows no values.

Security regressions (CLAUDE.md §46):
* attack 1 and 2 with a bank account: wrong origin, `github.com.evil.com`
  style hosts, and a frame on an unsaved top page are all DENIED;
* attack 3: a locked vault refuses a bank fill;
* a card, identity or note id sent to the bank path is DENIED, and a bank
  id sent to TOTP, passkey and SSO requests is DENIED;
* `auto_sign_in_for` is `false` for a bank account whatever the settings.

`check_login`: the second page's password equal to a saved bank account's
password is Unchanged.

Classification:
* the shared fixture runs on both platforms with the B6 gaps removed;
* new unit cases:
  * conta without agência stays non-bank;
  * "Agência de viagem" stays non-bank;
  * the bank menu on a site with only an old bank login lists that login,
    and a pick fills its username into the box;
  * "Entrar na sua conta" stays a login;
  * a 1-char box after conta is `bankAccountDigit`;
  * BB's 6-long branch box.

Writing (§5.2): table tests for each layout on both platforms:
* separate digit;
* hyphen fits;
* only len + 1 fits;
* `type=number`;
* no digit saved.

Extension jsdom: Itaú, Bradesco and BB forms from the fixture filled by one
pick; a password-only second page offers the bank item. Android: dataset
building for the same three.

Import: a 1PUX bank account with routing, SWIFT and an out-of-range value.

UI (`ui:check`, both languages and themes): list, detail with conta
revealed, editor with the split boxes; Android screenshots of the same. Per
the owner's rule, both get an **impeccable** review before done.

Then `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`, `tsc`,
`vitest`, Android unit tests, the field-words CI check and `cargo audit`. No
new dependency is expected.

## 9. Order of work (for the plan)

1. Core: type, fields, overview, binding, fill reply, tests (incl. security
   regressions).
2. Shared words + extension classifier, menu, writing, popup.
3. Desktop UI and commands; import/export.
4. Android: classifier, datasets, item screens.
5. Docs, CLAUDE.md note, roadmap B6 → done.

Extension and Android ship together or close together (shared word file).
Every device needs updating before a bank account is created (§4.7).

## 10. Left to the builder

* How the site-bound check is shared between Login and BankAccount.
* Component reuse between the login and bank editors on desktop and Android.
* The exact score weights for the bank roles, as long as the fixture and the
  negative cases pass.
* Glyph drawing for the bank icon (added to `Icon.tsx` and `HavenIcon.kt`
  together).

## 11. Revisions during implementation (2026-11-03/04)

* §5.3: the popup has no "Copy password"; it mirrors logins (Fill only, and
  nothing while frozen).
* §5.3: with nothing saved, the bank menu opens only from the icon, not on
  focus.
* §6.1: copy buttons on bank, holder, agência, conta and password (not type),
  and a "Copy details" action composed in Rust; `copy_bank_field` allows
  `bank`, `holder`, `details` as well.
* §6.1: detail and editor order Bank, Agência, Conta, Type, Holder, Password;
  the password label is "Password" / "Senha"; the English subtitle is
  "Branch …".
* §4.3: `accountLast3` is the last 3 characters of account plus digit.
* §6.1: typing never splits a number; only a paste does.
* §6.2: the phone keeps the saved account behind Change / Keep / Remove and
  notes behind an eye.
* §5.4: Android bank fill overwrites existing values like Android login fill.
* §5.1: unknown `autocomplete` tokens are neutral; a digit word makes a digit
  box only with no `maxlength` or 1-2; the first box of a one-time-code run
  is never a digit; boxes with a bank role are refused by the login and
  identity classifiers.
