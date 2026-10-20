# B5 — Shared field words and Brazilian label fixes

Status: **Implemented** (2026-10-19, branch `feat/shared-field-words`). The "My call" notes are decisions made at the builder check; implementation rulings are in the commit history.
Roadmap: `docs/autofill-roadmap.md` B5.

## 1. Why

The owner runs into forms the extension gets wrong. When a site words a label a
little differently, especially in Portuguese, the field gets no menu and the
owner copies and pastes by hand. Two problems cause this:

1. **Two classifiers drift apart.** The extension's TypeScript
   (`apps/extension/src/autofill/{classify,card-kind,identity}.ts`) and
   Android's Kotlin
   (`apps/android/.../autofill/{FieldClassifier,CardFieldClassifier,IdentityFieldClassifier}.kt`)
   each carry hand-copied keyword lists, and the copies already differ.
   Android dropped `address` from the not-a-username list and the extension
   never did, so Drogasil's "E-mail ou CPF" box (`name="CPF or Email Address"`)
   gets no menu in the browser.
2. **Word lists only match spellings someone wrote down.** For example,
   `normalize("C.P.F")` is `"c p f"`, which matches nothing.

A survey of 35 Brazilian sites (§6) found six concrete misses (§4).

## 2. Goals, non-goals, anti-goals

**Goals**

- One reviewed file holds every keyword list and scoring weight. The
  TypeScript and Kotlin copies are generated from it, and CI fails when a
  generated copy is stale.
- One text-cleanup rule, identical on both platforms, that also joins dotted
  abbreviations.
- One shared set of labelled real-world fields (§6). Both test suites run it,
  so the platforms can't drift silently.
- Fix the six misses in §4.

**Non-goals**

- Moving the classification logic into Rust or WASM. The owner chose
  "share the data, not the code" (option 2).
- Changing form grouping, auto sign-in, save detection or the desktop app.
- iOS. It will consume the same generated file when it starts.
- Bank agência/conta fill. The owner moved it to a later plan that adds a
  Bank Account item type (roadmap B6). The survey's bank records stay in
  the fixture as `other`, ready for that plan.

**Anti-goals** (failure even if it works)

- Anything new running inside web pages beyond the existing content script.
  No WASM, and no CSP change.
- Any change to what Rust returns to the extension or to Android: no
  protocol or core change.
- Losing a classification that works today. The existing extension and
  Android tests must still pass unchanged, except where a test encodes a
  miss fixed here.

## 3. Shared word file

### 3.1 Location and format

`packages/field-words/words.json` is new. It's plain data, reviewed like
code. It holds:

- **Keyword lists, by name.** Every list now hard-coded in `classify.ts`,
  `card-kind.ts`, `card-fill.ts` (months, brand aliases), `identity.ts`,
  `text.ts` (submit and pay words), and their Kotlin twins.
- **Scoring weights and thresholds.** One value where both platforms agree
  today. Where they differ (Android's username threshold is 40, the
  extension's 50, plus a 70 "username alone" threshold Android lacks), the
  file holds a per-platform value. *My call:* aligning them is not part of B5,
  because moving Android to 50 would silently drop fields it fills today.
- **Autocomplete token sets**, for example `AC_NOT_LOGIN`.

Each phrase is stored already normalized (§3.3). A list may carry a short
comment field. The schema is left to the builder, as long as it's
flat and readable in review.

**Step 1 is no behaviour change.** The first generated copies reproduce
each platform's current lists exactly, with per-platform entries where they
differ. Merging comes after, as its own TDD step. *My call* on the merge:
take the **union** of positive words and the **union** of negative words
(so Android keeps `buscar`, `pesquisar` and `cupom`, and the extension gains
them), with the §4.1 exception for `address` applied on both platforms. Any
existing test whose expectation changes is listed in the plan.

### 3.2 Generation

A small Node script, `packages/field-words/generate.mjs`, writes:

- `apps/extension/src/autofill/words.generated.ts`
- `apps/android/app/src/main/kotlin/net/havenkeys/android/autofill/Words.kt`
  (generated)

No Rust output for now. The generator is written so a Rust target can be
added when a core feature needs the words (B6 may).

The generated files are committed, with a "generated, do not edit" header.
`pnpm check:field-words` re-runs the script and fails if anything changed.
*My call:* CI has no JS lint step, so a new small workflow
(`.github/workflows/field-words.yml`, on push and pull request, Node only)
runs it. Committing the output keeps the Android
Gradle build and Cargo free of a Node dependency.

The classifiers import their lists and weights from the generated files
instead of declaring them. The scoring *logic* stays per platform, but
reads only generated constants.

### 3.3 Normalizer

The rule is the same on both platforms (TS `text.ts`, Kotlin `TextMatch.kt`):

1. Cut to the per-source character cap (unchanged).
2. Split camelCase (`loginEmail` → `login Email`).
3. **New:** join dotted single letters. A run of two or more single letters,
   each followed by `.`, becomes one word: `C.P.F` / `C.P.F.` / `c.p.f` →
   `cpf`, `R.G.` → `rg`, `C.N.P.J.` → `cnpj`. A lone `E.` or a word such as
   `Sr.` is unchanged.
4. Remove accents (NFKD, drop combining marks), lowercase, and turn every
   non-`[a-z0-9]` run into one space, then trim.

The shared fixture carries `normalize` test vectors (input → output),
including the dotted cases, `"Usuário"` → `"usuario"` and
`"E-mail / Login"` → `"e mail login"`. Both platforms assert them.

## 4. Fixes found in the survey

Each fix is a fixture record (§6) that fails before the change and passes
after.

1. **The username "address" penalty.** `address` must not count against a
   username when the same text names email (`email address`, `e mail address`,
   `endereco de e mail`). Drogasil's `name="CPF or Email Address"`,
   "E-mail ou CPF", must score as a username.
2. **Operator code.** "Código do operador" (Itaú) is a username, not a
   one-time code. `operador` and `codigo do operador` join the strong username
   words; `codigo do operador` joins the OTP negative words.
3. **A bare "Código" one-time-code box.** Ticketmaster's `name=code`, label
   "Código", placeholder `------` must clear the OTP threshold. A placeholder
   made only of 4–8 `-`, `_`, `•` or `0` characters is a new OTP signal, and
   `codigo` counts strong when the field has no username, password or
   card words. The builder picks the weights so the §6 fixture passes and no
   existing test regresses.
4. **`type=tel` is not always a phone.** In the identity classifier,
   `type=tel` stops short-circuiting to `phone`. Words decide first
   (birth date, CEP, house number, CPF), and `tel` only breaks a tie toward
   phone. Globo/TETO "Data de nascimento" with `dd/mm/aaaa`, TETO "CEP" and
   TETO "Número" (house number) must classify correctly. The same applies
   to Android's `IdentityFieldClassifier`.
5. **City and state vs. street.** The visible label wins over generic
   attribute words. When the label names a specific part (`cidade`,
   `estado`, `bairro`, `complemento`, `cep`), a generic `endereco` or
   `address` in `name`/`id` does not make it `street`. Globo's
   `enderecoAutoComplete` "Cidade" and `endereco.estado.nome` "Estado", and
   TETO's `address_attributes[city]` "Cidade", must classify as city or
   state.
6. **Cardholder name.** "Nome descrito no cartão", "nome impresso no
   cartão", "nome no cartão" and "titular do cartão" are the card name
   (weak card mode included). Such a field must never score as a username.

## 5. Security notes

- No new data leaves Rust and no protocol changes. This only changes which
  fields get the existing menu and fill.
- `words.json` becomes security-relevant input to the classifiers: a bad
  edit could make a password box look like a username, or a card box look
  like an identity field. It's reviewed like code, and the generated-copy
  check prevents edits that bypass review. Record this in
  `docs/security-model.md` (autofill section).

## 6. Shared fixture

`packages/field-words/fixtures/br-fields.json` holds the 137 records from
the 2026-10-19 survey (35 sites). It comes from the scratchpad `labels.json`
with the rendered page dumps dropped. Each record keeps: site, page kind,
tag, type, name, id, placeholder, aria-label, autocomplete, inputmode,
maxlength and label text.

*My call* on expectations: each record carries **one expectation per
classifier**, `login`, `identity` and `card`, each either the classifier's own
role name or `none`. One field can rightly be claimed by two classifiers:
a CPF login box is `login: username` and `identity: cpf`, a signup e-mail is
`login: username` and `identity: email`, and a newsletter box is
`login: none` and `identity: email`. The survey's coarse labels (`address`,
`name`, `cpf-as-username`…) are relabelled by the builder from each record's
label text, and that reading is authoritative. Agência, conta and dígito
boxes, and the survey's `other` records, are `none` on all three until B6.

- The extension test builds each record as a jsdom input with its label.
  The Android test builds `FieldFacts`. Each asserts the role its
  classifiers give **for that field on its own**. Records whose true role
  depends on group context (for example a username that only wins as "last
  field before the password") carry a `context` hint the test applies.
- A record a platform can't classify yet goes in a `known_gaps` list in the
  fixture, with a one-line reason, naming the platform and classifier.
  *My call:* B5 must fix the six §4 misses and the normalizer. Any other
  failure the relabelling turns up may go to `known_gaps` rather than grow
  B5. The plan lists them so the owner sees what's deferred.
- The fixture is also the template for adding sites later: one record per
  field, no page HTML.

Thin areas, honestly: one real one-time-code field and two card forms,
both donation pages. 2FA screens and store checkouts need a signed-in
session, which the survey didn't use. The existing English fixtures in
both test suites still cover those.

## 7. Left to the builder

- The JSON schema of `words.json` and the generator's structure.
- Exact weights for §4.3, within "the fixture passes and nothing regresses".
- How the Kotlin and TS scorers are refactored to read generated constants,
  as long as the scoring logic stays reviewable per platform.
- Order of work. Suggested: the word file and generator with no behaviour
  change (all tests green), then the normalizer, then the §4 fixes one by
  one under TDD.

## 8. Done when

- `pnpm check:field-words` passes, and fails when a list is edited without
  regenerating.
- Every §6 record passes on both platforms or sits in `known_gaps` with a
  reason.
- All six §4 fixes are covered by failing-first tests.
- `docs/autofill.md` and `docs/security-model.md` are updated, the roadmap marks B5 done, and the impeccable check is skipped
  because there's no UI change.
