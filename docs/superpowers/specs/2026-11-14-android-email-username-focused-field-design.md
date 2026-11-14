# Android Autofill — email as username, and the focused field's group

Status: **Implemented** (2026-11-14, branch `feat/form-detection-parity`), not yet run on a device.
Related: `docs/android.md` → "Login fields: the focused form and email boxes";
`apps/extension/src/autofill/group.ts` (the grouping this copies).

## 1. Why

1Password ships two Android autofill behaviours behind the feature flags
`android-autofill-email-as-username` and
`android-autofill-focused-field-analysis`. HavenKeys falls short of both:

1. **Email boxes in a sign-in.** Android apps and pages mark the sign-in
   box as an email address (`View.AUTOFILL_HINT_EMAIL_ADDRESS`,
   `type="email"`, `autocomplete="email"`, `emailAddress`). The login
   classifier already scores those as a username (`SharedWords.USERNAME_HINTS`
   holds `emailaddress` and `email`; an email input adds 80), but the router
   reads the whole screen. An email-only first step on a page that also has a
   newsletter form (name + email) has two identity fields "in its frame", so
   the request went to the Identity, and no saved login was offered for the
   first step.
2. **Every field is analysed.** `FieldFacts.focused` was recorded, but the
   router and the login finder looked at every field of the frame. On a page
   with a sign-in and a newsletter (or a sign-up, or a search box) the
   focused newsletter box could become the login's username (the focused
   username wins in `LoginFormFinder.usernameOf`), and the login's rows were
   built for fields of other forms.

## 2. Goals, non-goals, anti-goals

**Goals**

- A request is answered for the focused field's own form or view group.
- An email box with login context is the login's username: offered the
  site's logins, saved as the username.
- An email box without login context keeps the Identity (one tap fills the
  identity's email), as before.
- No request with nothing focused changes.

**Non-goals**

- Cards: a checkout's boxes sit in several processor frames, so cards keep
  the whole screen (`CardFormFinder`, unchanged).
- Word lists: `packages/field-words` is untouched. The few Kotlin-side words
  (§3.3) can move there later.
- Saving: `SaveRequestReader` still reads the whole screen; only the
  username choice (§4.4) changes.

**Anti-goals** (failure even if it works)

- Moving any fill decision into Kotlin. Rust (`havenkeys-mobile`) still
  decides the target, the matches and every value; direct fill and "Confirm
  before filling" are unchanged (CLAUDE.md amendments of 2026-10-01 and
  2026-10-02). This change only decides which fields a response covers.
- A sign-up or identity form answered with a login fill.
- A fill that loses the password it had before (a native view group that
  splits a username from its password).

## 3. Email as username

### 3.1 Rule

An email box (or any username box) alone in its group, with no password,
code or bank box there, is a login's first step only with **login context**
(`LoginFormFinder.find(group, screen)`):

1. nothing in its frame outside the group signs in (no password, new
   password or code field) and its words do not say it is for something
   else (§3.3); or
2. the whole screen, read as if nothing were focused, pairs this very box
   with a password in another form (a username form apart from its
   password form).

Otherwise it is not a login's box: no login is found for it.

An email box beside a password in its group is the username exactly as
before (`FieldClassifier` scores it; `usernameOf` picks it).

### 3.2 Routing

`FormRouter.route` finds the login and the identity in the focused group:

- login found and the focused box is its username → **Login** (unless the
  group is an identity form, unchanged rules below);
- no login, and the focused box is an email box alone → **Identity** with
  that one field (`IdentityFormFinder.find(loneEmail = true)`): a newsletter
  box gets the identity's email;
- an identity form (two or more identity fields in the group) without a
  sign-in owning the focus → **Identity**, as before. A sign-up (new and
  confirmation passwords) has no login (`LoginFormFinder` returns null) and
  keeps its "Save password?" sheet.

Without groups (no form, no view group: §4.3) the screen is one group, so a
first-step email box next to a page's other identity fields still goes to
the Identity: without structure there is no telling a contact form from a
first step, and the Identity is the conservative answer.

### 3.3 Words that say "not a sign-in"

`LoginFormFinder.NOT_A_SIGN_IN` (Kotlin-side): newsletter, subscribe,
subscription, unsubscribe, search, query, coupon, promo, comment, feedback,
message, invite, referral, gift, buscar, pesquisar, cupom, assinar,
inscreva, inscrever, novidades, boletim. A lone username box whose name,
id, hint, placeholder or label carries one is no first step, however it is
hinted. The shared `USERNAME_NEGATIVE` list is not reused as-is: its "code"
would refuse "we'll email you a sign-in code", a real first step.

**My call:** keep this list in Kotlin for now (another change owns
`packages/field-words`); moving it into `words.json` as
`NOT_A_SIGN_IN` for both platforms is a follow-up.

## 4. The focused field's group

### 4.1 What a group is

`StructureParser` numbers every node it visits and keeps, for each field,
its nearest HTML `<form>` ancestor and its nearest ancestors (bounded).
`FieldGroups.assign` then gives each field a `FieldGroup`, the extension's
`groupRoot` rule:

- **Web form** (`FieldGroup.form = true`): the nearest node whose
  `htmlInfo.tag` is `form`, when it holds at most 40 fields
  (`MAX_FORM_INPUTS`; a page-wide ASP.NET form is no group).
- **View group** (`form = false`): otherwise the nearest ancestor, at most 8
  levels up (`MAX_CLIMB`), that holds another text field (lists and dates
  do not count), never one holding more than 40 fields.
- **None**: a field with no such ancestor; its group is the whole screen.

Work stays linear (each field counts itself into at most 8 ancestors), so
CLAUDE.md attack 7 holds; `FieldGroupsTest.thousandsOfGroupedFieldsRouteFast`
checks it.

What browsers show: WebView and Chrome give the page's fields under a
`form` node (and send one form per request); Firefox (GeckoView) gives each
`<form>` as a node. Compatibility-mode browsers (accessibility) give no
forms, so their fields get view groups.

### 4.2 Which fields a request reads

`FieldGroups.focusedGroup(fields)`:

- nothing focused, or the focused field has no group → **every field**
  (the behaviour before this change);
- a web form → **that form's fields**;
- a view group → its fields **only when** it holds a password or code of
  its own, or the focused field's frame has none outside it; otherwise
  every field.

**My call — why native view groups are trusted less than forms:** Android
layouts wrap each box (`TextInputLayout > FrameLayout > EditText`) and put
boxes in rows (country code + phone). The climb stops at the first container
with a second text field, which can be a row that holds the username but not
the password. A fill must never lose the password it had, so a view group
is used only when it is a whole sign-in or when nothing else on the screen
signs in. Native screens rarely show two forms; when they do (an app's
newsletter box), §4.4 still keeps the newsletter box out of the login.

### 4.3 Consequences

- A newsletter or search form's focus no longer builds login rows for the
  sign-in elsewhere on the page; Android asks again when the user moves into
  the sign-in.
- Two sign-ins on one page (personal and business): the one the user is in
  is filled.
- A password alone in its web form, with its username in another form, is
  filled without the username (the extension does the same). A username
  alone in its form, with the password in another form, is paired (§3.1).

### 4.4 The username beside the password

Whatever the group, `LoginFormFinder.usernameOf` now picks the username
among the candidates in the password's group first (falling back to every
candidate before the password when none is there). A focused newsletter box
outside the password's group is therefore never the login's username, on
the whole-screen path too (native screens, saves).

## 5. Security

- No Rust or protocol change. Kotlin still sends Rust only the target
  (package, certificates, the frame's web domain and scheme); which fields a
  dataset covers was always Kotlin's and is narrowed here, never widened.
- Fields of another frame are still never filled (`LoginFormFinder`'s frame
  check is unchanged and runs inside the group).
- The structure is untrusted: groups only narrow what is offered. A hostile
  page can claim any form structure, which at worst puts its own fields in
  one group, as it could already put them on one screen.
- Nothing new is logged or kept; `FieldFacts` still holds no value.

## 6. Tests

JVM unit tests (`apps/android/app/src/test/.../autofill/`):

- `FieldGroupsTest`: forms, page-wide forms, the climb (wrappers, lists, the
  8-level limit), the focused group and its native fallback, scaling.
- `LoginFormFinderTest`: hinted email beside a password; newsletter email
  next to a sign-in (worded, unworded, above the sign-in); a focused app
  newsletter box outside the password's view group; username and password
  in separate forms; email first step in its own form; a lone newsletter
  email; "email you a code" stays a first step; a search box beside a
  sign-in.
- `FormRouterTest`: an email first step beside a newsletter form goes to
  logins (with its save); without groups it stays the Identity; an identity
  form without a password stays the Identity; a newsletter (with a name, or
  alone) next to a sign-in gets the Identity; a sign-up beside a sign-in
  gets the Identity and its save, never a login; a search box routes
  nowhere; the focused sign-in of two is routed.
- `FillPlannerTest`: the focused form's datasets cover only its fields;
  nothing focused plans the whole screen as before; an email first step is
  planned as the login's username.

## 7. Open questions

1. Should a password alone in its web form borrow a username from another
   form (today's whole-screen behaviour) instead of following the
   extension? Kept with the extension for now.
2. Move `NOT_A_SIGN_IN` into `packages/field-words/words.json` and use it in
   the extension's username-only step too.
3. Device check: confirm on Chrome, Firefox and a WebView app that the
   `form` node and focus arrive as expected (part of the manual checklist).
