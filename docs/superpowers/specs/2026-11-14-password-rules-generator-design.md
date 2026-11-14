# Passwords that follow a site's rules — design

Date: 2026-11-14. Status: implemented (pending review).

## 1. Goal

The in-page "Generate strong password" row fills a password made by the
desktop's generator with the policy saved in its generator tab (or the one
set in the menu's panel). It ignores the field: a site that allows at most
16 characters, or only some symbols, or demands a digit, rejects the
password, and the user has to edit it by hand.

1Password solves this with the HTML `passwordrules` attribute Apple
proposed (parsed by its own `password-rules-parser` crate) and with Apple's
table of known sites' rules. HavenKeys now does the same: a password
generated for a field follows what that field, or the site, asks for. With
no rules and no length attributes, nothing changes.

## 2. Decisions taken

| Question | Decision |
|---|---|
| Sources, highest priority first | (a) the field's `passwordrules`; (b) a bundled per-site table; (c) the plain policy. The field's `minlength`/`maxlength` always bound the length too |
| A source that does not parse or cannot be met | Skipped for the next one (field rules → site rules → plain policy). Never an error where generation succeeds today |
| Parser | `password-rules-parser = "1.1.0"` (1Password; MIT OR Apache-2.0). The grammar is not hand-rolled |
| Bundled table | Apple's `password-manager-resources` `quirks/password-rules.json` (MIT), converted to `crates/havenkeys-core/data/password-rules.json`, compiled in with `include_str!`, never fetched |
| Table lookup | The page's host, else its registrable domain (Public Suffix List, `origin.rs`) |
| Which URL | The frame that holds the field (`url`), from the browser's sender data. Not the top page: a sign-up form in an identity provider's frame follows that provider's rules |
| Length | The policy's length clamped into `[max(minlengths), min(maxlengths, 128)]`; at least 8 unless the page's maximum is lower; never below 4 (the floor) |
| Character set | The union of the `allowed` and `required` classes; printable ASCII only, never the space; `unicode` reads as ASCII printable |
| The user's class switches | Ignored when rules are followed (the rules say which classes); `avoidAmbiguous` still applies when it leaves every class non-empty |
| Where it runs | Rust (`havenkeys_core::password_rules`), as the generator always has |
| Transport | Optional `constraints` on `generate_password` and `generator_options`; the results gain an optional `followed` |
| Older desktop | Rejects `constraints` as `malformed`; the extension retries `generate_password` without them, and shows no note |
| Menu | The generate row says "Follows this site's password rules" when Rust says it will (asked when the menu opens) |

## 3. Core (`crates/havenkeys-core/src/password_rules.rs`)

```rust
pub struct FieldConstraints<'a> { rules: Option<&'a str>, min_length: Option<u32>, max_length: Option<u32> }
pub enum RuleSource { Field, Site }
pub fn generate_for_page(policy, page_url, field) -> Result<PagePassword>   // { generated, followed }
pub fn followed_for_page(policy, page_url, field) -> Result<Option<RuleSource>>
pub fn site_rules(page_url) -> Option<&'static str>
pub const RULES_MIN_LENGTH: usize = 4;
```

### 3.1 Plan

For one rules string (the field's, then the site's):

1. Longer than 1024 bytes, or `parse_password_rules(rules, true)` fails:
   no plan.
2. `min` = the larger of the rules' and the field's `minlength`; `max` = the
   smaller of their `maxlength`s, capped at 128.
3. `hi = max` (128 without one); `lo = max(min, min(8, hi))`. If `hi < 4`
   or `lo > hi`: no plan. `length = policy.length.clamp(lo, hi)`.
4. Each `required` group becomes the set of its classes' characters
   (printable ASCII but space). An empty group, or more groups than
   `length`: no plan.
5. The alphabet is every `allowed` and `required` character. Empty: no plan.
6. `avoidAmbiguous`: remove `Il1O0o` from every set, unless that empties one.
7. `max-consecutive: n` applies when `0 < n < length`; with one character
   in the alphabet it cannot be met: no plan.

### 3.2 Generation

`generator::build(required, alphabet, length, max_consecutive)` is the
generator `generate` already used, with the run limit added: one character
from each required set, the rest from the alphabet (`rand::distr::Uniform`,
rejection sampling, no modulo bias), then a Fisher–Yates shuffle with the
same unbiased sampler. With a run limit, a character that would make a run
longer than `n` is drawn again from the alphabet without the repeated
character; a result that thereby lost a required class is discarded and
built again (64 attempts, then the next source). `generate` itself is
unchanged in behaviour (`build` with its four classes and no run limit).

### 3.3 Plain fallback

Without a workable plan, the policy's classes at the policy's length,
moved into the field's `[minlength, maxlength]` when that range is workable
(the same `lo`/`hi` rule, and at least as long as the number of classes);
otherwise exactly `generate(policy)`.

### 3.4 Table

`data/password-rules.json` is `{ "rules": { "<domain>": "<rules>" } }`,
rebuilt by `scripts/update-password-rules.mjs` from a local copy of Apple's
file. A table that does not parse reads as empty (generation is then the
plain policy). Tests require every entry to be a lowercase hostname, to
parse, and to produce a password that meets its own rules.

## 4. Protocol

| Message | Change |
|---|---|
| `generate_password` | `constraints?: { url, rules?, minLength?, maxLength? }` |
| `generator_options` | the same `constraints?` |
| `generate_password` result | `followed?: "field" \| "site"`, sent only when rules were followed |
| `generator_options` result | `followed?`, the same: which rules a generate with these constraints would follow |

Bounds (Rust `havenkeys-protocol` and `packages/protocol`): `url` an
`http://` or `https://` URL of at most 4096 bytes; `rules` 1–1024 bytes;
`minLength`/`maxLength` integers 1–1024. Unknown keys are rejected. A result
without `followed` has no key at all, so an older extension (which never
sends constraints) parses the answers exactly as before. Both requests stay
in the lookup rate class.

## 5. Extension

* **Content script** (`autofill/password-rules.ts`, called from
  `content/index.ts` for a `new_password` menu): reads `passwordrules`
  (trimmed; over 1024 bytes it is dropped whole, never cut) and the
  `minLength`/`maxLength` IDL values (kept only in 1–1024), and sends them as
  `passwordRules` in `cs_open_menu`.
* **Background** (`background/inline-handler.ts`): `parseContentRequest`
  accepts `passwordRules` only for `new_password`, with the protocol's
  bounds. The URL is the frame's from the browser. When the menu opens
  (unlocked), it asks `generator_options` with the constraints; any error is
  no note. `menu_generate` sends `generate_password` with the constraints;
  on `malformed` or `invalid_input` it sends the same request without them.
* **Menu** (`menu/menu.ts`): the generate row's second line reads "Follows
  this site's password rules" / "Segue as regras de senha deste site" when
  the view carries `rules`. The password still never passes through the
  menu.

The note is computed before the user picks, because the menu closes when the
password is filled (the content script finds the field through the menu it
just closed). `followed` on the `generate_password` result is the
authoritative answer and is carried for completeness; it can differ from
the note only if the user's panel changes the length so that a rule's
required groups no longer fit, which the bundled table never does.

## 6. Security

Every value here (the rules, the lengths, the page whose site picks the
table entry) is controlled by the page that receives the password. A hostile
page can therefore only weaken its own password: make it short (never below
4 characters, and below 8 only through its own `maxlength`), drop classes,
or narrow the alphabet. It cannot read anything, and cannot affect a
password generated for another page. The URL comes from the browser, so a
page cannot pick another site's table entry. Requests are bounded at every
hop, the rules string never appears in `Debug` output, and nothing is
logged. The parser is a small `nom` grammar run on at most 1024 bytes.
See `docs/security-review.md`, "Passwords that follow a site's rules".

## 7. Tests

* `password_rules::tests`: each class alone, required groups (a group is a
  union), custom classes, `max-consecutive`, length clamping (rules and
  field), the floor, unparsable and unsatisfiable rules falling back, plain
  policy bounded by the field, host then registrable-domain lookup
  (including `163.com.evil.example`), field over site priority,
  `avoidAmbiguous`, and every bundled rule parsing and being met.
* `havenkeys-protocol/tests/messages.rs`: constraints parse and re-encode,
  shape and bounds rejections, `followed` results, `Debug` hygiene, fuzz
  seeds.
* `havenkeys-bridge/tests/bridge.rs`:
  `generate_password_follows_the_fields_rules_then_the_sites`.
* `packages/protocol/src/password-rules.test.ts` and fuzz seeds: field-rules
  validator, `followed` parsing, bounds equal to Rust's.
* Extension: `autofill/password-rules.test.ts` (attribute reading and
  bounds), `content/content.test.ts` (sent with the menu request),
  `background/inline-handler.test.ts` (constraints, the note, no note on
  error or when locked, retry on an older desktop, no retry otherwise,
  message validation), `menu/menu.test.ts` (the note).
