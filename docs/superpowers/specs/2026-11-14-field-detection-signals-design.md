# Field detection signals: aria state, neighbouring text, test hooks

Status: **Implemented** (2026-11-14, branch `feat/form-detection-parity`), extension only.
Origin: comparison with 1Password's page-details model (form-detection parity work).

## 1. Why

1Password's page-details model records, for every field, `isAriaDisabled`,
`isAriaHidden`, `labelBefore`/`labelAfter` and `dataAttributes`. Comparing it
with ours showed three gaps in the extension's field detection
(`apps/extension/src/autofill/`):

1. **aria state.** `isFillable` (`usable.ts`) checked `disabled`, `readOnly`,
   the type and visibility. A field marked `aria-disabled="true"`, or placed
   in an `aria-hidden="true"` subtree, was classified, counted and filled.
   Honeypot inputs are often hidden from screen readers this way while
   still rendered, so a page could make a decoy username or password box
   take part in a login group.
2. **Neighbouring text.** `labels.ts` found linked `<label>`s and floating
   labels; `group.ts` added placeholder, aria-label, title and
   `aria-labelledby`. Plain text placed next to a field
   (`<td>Username:</td><td><input></td>`, `<span>Password</span><input>`)
   was never read, so old table layouts and span-labelled forms had nothing
   but `name`/`id` to go on.
3. **Test hooks.** `data-testid` and friends often name a field better than
   a generated `name`/`id` (`name="f_3a9"`, `data-testid="login-email"`), and
   were ignored.

## 2. Goals, non-goals

**Goals**

- One aria rule for every fillability decision: login, card, identity, bank,
  and the write itself.
- A bounded, conservative "text before/after" label source that never takes
  text belonging to another control.
- A fixed allowlist of test-hook attributes feeding the username and OTP
  scores, weaker than `name`/`id`.
- Scoring stays scoring (CLAUDE.md §21): no new signal decides alone.

**Non-goals**

- Android. Its classifier reads `AssistStructure`, which has no aria state
  or DOM siblings. The new weights are extension-only entries in
  `packages/field-words/words.json`, so `SharedWords.kt` is unchanged.
- Password-role wording from test hooks (§5).
- Protocol, Rust, generator: untouched.

## 3. aria-disabled and aria-hidden

New module `autofill/aria.ts`, no imports:

- `isAriaHidden(el)`: the field or an ancestor has `aria-hidden="true"`
  (trimmed, case-insensitive). aria-hidden is inherited by the whole
  subtree, so the walk goes up through `parentElement` and across shadow
  roots to the host. Bounded by `MAX_ARIA_ANCESTORS` (128); past that the
  field counts as not hidden, which is today's behaviour.
- `isUnavailable(el)`: `disabled`, `:disabled` (so a `<fieldset disabled>`
  around the field counts, as it already did for buttons in `submit.ts`),
  `aria-disabled="true"` on the field, or `isAriaHidden`.

`aria-disabled` is read on the field only. ARIA 1.2 lets it apply to
focusable descendants of a container, but ARIA 1.3 narrows that and
browsers do not block typing in either case; the field's own attribute is
the clear signal.

Callers: `usable.ts` `isFillable` (and through it `usableInputs`,
`isStrictlyFillable`, `setValue`, `page.ts`, `content/index.ts`,
`content/rerender.ts`), `card.ts` `isCardFillable`, `identity.ts`
`isIdentityFillable`, `bank.ts` `wordsOf`. `watch.ts` also observes the two
attributes so a step that un-hides is seen.

## 4. Neighbouring text

`labels.ts` `neighbourText(el)`, used by `labelText` only when the field has
no linked `<label>` and no floating label. So it reaches every classifier
that reads `labelText`: login (`group.ts`), card (`card-kind.ts`), identity
and bank. It counts as label text (+40 for a strong username word): when a
page has nothing else, the text beside the field is what the user reads as
its label.

Reading order: the field's previous siblings, then (when they ran out with
nothing) the previous siblings of its wrapper, up to two wrappers. A wrapper
qualifies only when nothing in it besides the field is a control, and the
climb never goes above a `<td>`/`<th>`, so a cell's preceding cell is read
but never the row above. If nothing is found before, the same is done after
the field.

Ownership rules, so another field's label is never used:

- Text between two controls belongs to the one after it. Text after the
  field counts only when no later sibling holds a control (checked within
  a 60-element budget; out of budget counts as "holds one").
- A control, a `<label>` linked to another control, or a non-label element
  (heading, link, button, legend, select, table, list, form, media…)
  between the text and the field ends that side.
- Hidden (`hidden`, `aria-hidden`) and decorative siblings are read past:
  a hidden decoy cannot become the label, and the visible text behind it
  still can.
- Text over `MAX_NEIGHBOUR_CHARS` (60) is prose and is ignored.

Bounds: 3 levels, 8 sibling nodes per side and level, 12 nodes inside one
sibling, 60 elements per control check. The text is only compared against
the fixed keyword lists, like every page string (CLAUDE.md §19).

## 5. Test hooks

`group.ts` `DATA_ATTRIBUTES`: `data-testid`, `data-test-id`, `data-test`,
`data-qa`, `data-cy`, `data-automation-id`. These are written by the site's
own developers to name a field for Testing Library, Playwright, Cypress, QA
tooling and Workday, so like `name`/`id` they describe the field's role in
words that are rarely localized. `data-name`, `data-field` and other
free-form attributes are left out: frameworks, analytics and A/B tools fill
them with anything. Each value is cut to 64 characters, the normalized
whole to 200, into `FieldFeatures.data`.

Weights (`words.json`, extension only):

| | Weight | Compared with |
|---|---|---|
| `USERNAME_WORD_IN_DATA` (strong) | 30 | name/id 50, label 40 |
| `USERNAME_WORD_IN_DATA_WEAK` | 15 | name/id 25, label 20 |
| `OTP_WORD_IN_DATA` (strong) | 40 | name/id/label 70, weak 30 |

- Username: counted only when `name`/`id` have no username word (no double
  count), and does not make the field "worded" for the username-only login
  bonus. Alone it reaches 30, below every threshold; beside the
  last-before-password signal (40) it reaches the 50 a field needs next to
  a password. Negative words in a hook (`newsletter-email`) apply −80.
- OTP: counted only when name, id and label have no strong OTP word, and
  in place of a weak one. Alone 40 < 60; with a 4–8 `maxlength` (15) and a
  numeric mode (10) it qualifies. `OTP_NEGATIVE` words in a hook disqualify.
- Password roles do not read hooks: `new`, `create` or `set` in a test id
  could flip a sign-in password to a new one, and position and intent
  already decide unworded passwords.

## 6. Tests

`autofill.test.ts`: aria-hidden honeypot not classified or filled; inherited
aria-hidden, through a shadow host; aria-hidden=false and an aria-hidden
sibling change nothing; aria-disabled and fieldset-disabled fields; a
password turned aria-disabled after classification is not written; the
ancestor walk bound. Neighbouring text: table cells, username-only step from
a cell, span and text-node labels, text after, text between two fields goes
to the second, never another control's label / heading / link / other row /
prose / wrapper with a second control, linked label wins, hidden decoys
skipped, bounds. Test hooks: weights, no double count, never alone,
username and OTP in a form, allowlist and length bounds.
`identity.test.ts`, `card.test.ts`, `bank.test.ts`: aria rule and table
labels for each classifier.

## 7. Open

- A page that leaves `aria-hidden="true"` on its app root after closing a
  modal (a known bug in some modal libraries) now gets no fill until it
  removes it. Accepted: the page says the content is not for the user, and
  the user can still copy the value from HavenKeys.
- `inert` subtrees are not checked; the user cannot focus or click into
  them, so the menu does not open there.
- Neighbouring text is not weighted below a real label. If false positives
  show up, split it into its own feature with a lower weight.
- Android has no equivalent of these signals.
