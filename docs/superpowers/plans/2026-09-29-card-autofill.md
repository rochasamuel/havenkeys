# Card Autofill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After the user picks a saved card in the HavenKeys menu or popup, the extension fills the checkout's card fields (top page, same-site frames and payment-processor frames) in whatever shape each field asks for, and offers to save a card the user typed into a checkout.

**Architecture:** Rust decides what leaves the vault. Three new native-messaging requests (`find_cards`, `fill_card`, `save_card`) are answered by a new core module, `card_page.rs`. It checks https, frame eligibility (same-site, or a fixed list of payment-processor origins) and the item, and returns whole values per role. The extension gets:

- a card classifier and writer, separate from the login and identity ones;
- a `card` menu kind;
- a background flow that finds the tab's card frames and fills them all with one `fill_card`;
- a top-frame host for menus opened inside small processor iframes;
- a save prompt for typed cards;
- a popup section.

**Tech Stack:** Rust (havenkeys-core, havenkeys-protocol, havenkeys-bridge), TypeScript (packages/protocol, apps/extension, MV3), vitest + jsdom.

**Spec:** `docs/superpowers/specs/2026-09-29-card-autofill-design.md`. Read it first; this plan implements it. It builds on `docs/superpowers/specs/2026-09-29-card-item-design.md`, whose plan (`docs/superpowers/plans/2026-09-29-card-item.md`) **must be merged before this plan starts**.

## Global Constraints

- `CLAUDE.md` rules apply:
  - Never log card values.
  - Never write values into attributes.
  - No `innerHTML`.
  - Every message is validated.
  - Rust enforces security, never the extension.
- Commits carry **no `Co-Authored-By` trailer** (user preference).
- Work on a branch `card-autofill`, cut from `main` after the card-item branch is merged. Merge back with `--no-ff` only if the user asks.
- Card roles (exact wire strings, camelCase, in this order): `cardholderName cardholderGivenName cardholderFamilyName number verificationNumber expiryMonth expiryYear brand`.
- Brand ids (wire, lowercase, in this order): `visa mastercard amex elo hipercard diners discover jcb unionpay maestro other`.
- `find_cards`:
  - at most 50 cards;
  - overview data only (id, title, brand, last4, expiry `YYYY-MM`);
  - `insecure: true` ⇒ `cards: []`.
- `fill_card`:
  - 1–8 frames, each with 1–8 unique roles;
  - the answer has one entry per frame, in the same order;
  - each value is 1–1024 bytes (`MAX_CARD_VALUE_BYTES`; see the spec corrections at the end).
- `save_card`:
  - `number` 1–64 bytes, `verificationNumber` 1–16 bytes, `cardholderName` 1–1024 bytes;
  - `expiry` matches `^\d{4}-\d{2}$`;
  - `title` is 1–1024 bytes.
- Rate-limit classes: `find_cards` = Lookup; `fill_card` and `save_card` = Secret.
- Rust (`card_page.rs`) checks, in order:
  1. The vault is unlocked and browser integration is on.
  2. The top page is https.
  3. Every frame is https **and** is either same-site as the top page or on `PAYMENT_FRAME_ORIGINS` / `PAYMENT_FRAME_PARENTS`. One bad frame ⇒ `Denied` for the whole request.
  4. The item is a Card, else `NotFound`: one answer for a missing item and for another type.
- Card fills never submit a form and never overwrite a non-empty value unless HavenKeys wrote that exact value.
- Save prompt:
  - Only for a card number the user typed in the **top frame**, passing the check digit, with an expiry.
  - Held in background memory only, for at most `CARD_SAVE_TTL_MS = 120_000`.
  - Survives the checkout's navigation, as login save prompts do.
- Every user-visible string goes in both `apps/extension/src/i18n/en.ts` and `pt-BR.ts` (the `Messages` type enforces the same shape).
- Names consumed from the card-item plan. Use them exactly; they are not redefined here:
  - `crates/havenkeys-core/src/card.rs`:
    - `CardBrand` (`Visa … Other`; `Copy + Eq`; `id(self) -> &'static str`, `display_name(self) -> &'static str`).
    - `CardExpiry { pub year: u16, pub month: u8 }` (`Copy`; `CardExpiry::parse(&str) -> Result<Self>`, `to_wire(self) -> String` giving `"2033-11"`).
    - `CardFields { cardholder_name: Option<SecretString>, brand: Option<CardBrand>, number: Option<SecretString>, verification_number: Option<SecretString>, expiry: Option<CardExpiry>, notes: Option<SecretString> }`, with `effective_brand(&self) -> CardBrand` (the user's choice, else detected, else `Other`).
    - `CardInput { cardholder_name, brand, number: SecretUpdate, verification_number: SecretUpdate, expiry, notes }`.
    - `IinRange { brand, lo, hi }`, `IIN_RANGES` (one row per line).
    - `detect_brand(&str) -> Option<CardBrand>` (first row whose equal-length `lo..=hi` holds the number's first `lo.len()` digits).
    - `check_digit_ok(&str) -> bool`, `clean_number(&str) -> Result<String>`.
    - `CardSummary { pub brand: CardBrand, pub last4: Option<String>, pub expiry: Option<CardExpiry> }` (`Clone`; zeroizes `last4` on drop).
  - `ItemOverview.card: Option<CardSummary>`, `ItemType::Card`, `ItemInput.card: Option<CardInput>`.
  - `VaultService::card_fields(&self, id: Uuid) -> Result<CardFields>` (`Denied` for a non-card).
  - `packages/ui/src/card-brand-icons.ts`: `CardBrandId` (the ten brands, no `other`), `CARD_BRAND_ICONS`, `GENERIC_CARD_ICON`, exported as `@havenkeys/ui/card-brand-icons`.
  - If one of these names or types differs in the merged code, adapt the call sites here. Do not change the card-item code.
- The server tests need Postgres (`scripts/test-server.sh`). The crate and unit tests below do not.

## Review Focus

1. **Card fields the checkout replaced between the scan and the fill.** Stripe remounts its iframes, and a SPA navigates. A frame whose tab top URL no longer equals the clicked frame's gets nothing. A frame whose document changed gets nothing, because `sendToFrame` pins the `documentId`. Pinned in Task 8 (`leaves out a frame from another top page`).
2. **A month `<select>` whose options read `"03 - Março"`, or whose first option is a `"Mês"` placeholder.** The right month is selected and the placeholder never is. Pinned in Task 6 (`month selects by number, name and mixed text; never the placeholder`).
3. **An Amex 4-digit code into a `maxlength=3` CVV field, and a CVV field of `type=password`.** The field is left empty, never truncated, and the password-typed CVV is still found. Pinned in Task 5 and Task 6.
4. **A card number field the site reformats as the user types** (spaces, dashes, a mask). The save prompt must still appear: the typed digits equal the field's digits. A value a page script wrote must still never be offered. Pinned in Task 9 (`offers to save a masked number the user typed`).
5. **A page with dozens of iframes reporting card-like fields** (ads, widgets). The scan records at most 16 reports and `fill_card` gets at most 8 frames. Each extra frame costs one lookup and is dropped if Rust denies it. Pinned in Task 8 (`caps the frames`).

---

## File Structure

**Rust**
- Create `crates/havenkeys-core/src/card_page.rs`: `CardRole`, `PAYMENT_FRAME_ORIGINS`, `PAYMENT_FRAME_PARENTS`, `is_payment_frame`, `CardOffer`, `CardList`, `CardFrame`, `NewCard`, and `impl VaultService { cards_for_page, card_values_for_page, stage_save_card }`.
- Modify `crates/havenkeys-core/src/lib.rs` (`pub mod card_page;`) and `src/identity_page.rs` (`truncate_bytes` becomes `pub(crate)`).
- Create `crates/havenkeys-core/tests/card_page.rs`.
- Modify `crates/havenkeys-protocol/src/lib.rs` (constants) and `src/message.rs` (`CardRole`, `CardBrandId`, `CardFrame`, `CardMatch`, `CardValue`, `CardFrameValues`, three requests, three results, validation). Tests go in `tests/messages.rs`.
- Modify `crates/havenkeys-bridge/src/dispatch.rs`, `src/server.rs` and `src/ratelimit.rs` (doc comment). Tests go in `tests/bridge.rs`.

**TypeScript**
- Create `packages/protocol/src/card.ts` and `src/card-parity.test.ts`.
- Modify `packages/protocol/src/index.ts`, `src/index.test.ts` and `tsconfig.json` (exclude the parity test).
- Create `apps/extension/src/autofill/card.ts` (classifier) and `card.test.ts`.
- Create `apps/extension/src/autofill/card-fill.ts` (writer and submission reader) and `card-fill.test.ts`.
- Modify `apps/extension/src/autofill/fill.ts` (`typedByUser`) and `autofill/text.ts` (`PAY_WORDS`).
- Modify `apps/extension/src/messaging/inline.ts`; create `messaging/inline-card.test.ts`.
- Create `apps/extension/src/background/card-rows.ts`.
- Modify `apps/extension/src/background/inline-handler.ts`; create `background/inline-card.test.ts`.
- Modify `apps/extension/src/background/index.ts`.
- Modify `apps/extension/src/content/index.ts`; tests go in `content/content.test.ts`.
- Modify `apps/extension/src/menu/icons.ts`, `menu/menu.ts`, `menu/save.ts` and `menu/inline.css`; tests go in `menu/menu.test.ts` and `menu/save.test.ts`.
- Modify `apps/extension/src/messaging/popup.ts`, `background/popup-handler.ts`, `popup/popup.ts` and `popup/popup.css`; tests go in `background/popup-handler.test.ts`.
- Modify `apps/extension/src/i18n/en.ts`, `pt-BR.ts` and `apps/extension/manifest/firefox.json`.

**Docs:** `docs/autofill.md`, `docs/native-messaging.md`, `docs/security-model.md`, `CLAUDE.md` (§25 note), `README.md`, `tools/ui-check/extension.mjs`.

---

### Task 1: Core — card access for web pages

**Files:**
- Create: `crates/havenkeys-core/src/card_page.rs`
- Modify: `crates/havenkeys-core/src/lib.rs` (add `pub mod card_page;` after `pub mod card;`)
- Modify: `crates/havenkeys-core/src/identity_page.rs:23` (`fn truncate_bytes` → `pub(crate) fn truncate_bytes`)
- Test: `crates/havenkeys-core/tests/card_page.rs`

**Interfaces:**
- Consumes: the card-item names in Global Constraints (`CardFields::effective_brand`, `CardExpiry::parse`); `crate::origin::{PageUrl, site_of, host_key}`; `crate::model::clean_title`; `VaultService::{session, list_items, stage_create}`; `crate::vault::StagedSave`.
- Produces:
  - `pub enum CardRole { CardholderName, CardholderGivenName, CardholderFamilyName, Number, VerificationNumber, ExpiryMonth, ExpiryYear, Brand }` (`Clone, Copy, Debug, PartialEq, Eq, Hash`).
  - `pub const PAYMENT_FRAME_ORIGINS: &[&str]`, `pub const PAYMENT_FRAME_PARENTS: &[&str]`.
  - `pub fn is_payment_frame(page: &PageUrl) -> bool`.
  - `pub struct CardOffer { pub id: Uuid, pub title: String, pub brand: Option<CardBrand>, pub last4: Option<String>, pub expiry: Option<CardExpiry> }`.
  - `pub struct CardList { pub insecure: bool, pub cards: Vec<CardOffer> }`.
  - `pub struct CardFrame<'a> { pub url: &'a str, pub roles: &'a [CardRole] }`.
  - `pub struct NewCard<'a> { pub title: Option<&'a str>, pub cardholder_name: Option<&'a str>, pub number: SecretString, pub verification_number: Option<SecretString>, pub expiry: Option<&'a str> }`.
  - `VaultService::cards_for_page(&self, page_url: &str, top_url: Option<&str>) -> Result<CardList>`.
  - `VaultService::card_values_for_page(&self, item_id: &Uuid, top_url: &str, frames: &[CardFrame<'_>]) -> Result<Vec<Vec<(CardRole, SecretString)>>>`.
  - `VaultService::stage_save_card(&self, page_url: &str, top_url: Option<&str>, card: NewCard<'_>, now_ms: i64) -> Result<StagedSave>`.

- [ ] **Step 1: Write the failing tests** (`crates/havenkeys-core/tests/card_page.rs`)

```rust
//! Cards for web pages (spec 2026-09-29-card-autofill §6.2, §6.3).

mod common;

use common::{activated_vault, login, secret, NOW};
use havenkeys_core::card::{CardExpiry, CardInput};
use havenkeys_core::card_page::{CardFrame, CardRole, NewCard};
use havenkeys_core::model::{ItemInput, ItemType, SecretUpdate};
use havenkeys_core::vault::VaultService;
use havenkeys_core::Error;
use uuid::Uuid;

const VISA: &str = "4111111111111111";
const SHOP: &str = "https://shop.example.com/checkout";

fn card_input(title: &str, number: &str, cvv: Option<&str>) -> ItemInput {
    ItemInput {
        item_type: ItemType::Card,
        title: title.into(),
        username: None,
        urls: vec![],
        password: SecretUpdate::Keep,
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Keep,
        auto_sign_in: None,
        sign_in_with: None,
        identity: None,
        card: Some(CardInput {
            cardholder_name: Some(secret("Samuel  S Rocha")),
            brand: None,
            number: SecretUpdate::Set(secret(number)),
            verification_number: cvv.map_or(SecretUpdate::Keep, |c| SecretUpdate::Set(secret(c))),
            expiry: Some(CardExpiry { year: 2033, month: 4 }),
            notes: None,
        }),
    }
}

/// A vault holding one Visa (with CVV) and one login.
fn with_card() -> (VaultService, Uuid, Uuid) {
    let (mut v, _) = activated_vault();
    let staged = v.stage_create(card_input("Visa pessoal", VISA, Some("123")), NOW).unwrap();
    let card = v.commit_write(staged, 1).unwrap().unwrap().id;
    let staged = v
        .stage_create(login("GitHub", "octo", "pw", "https://github.com"), NOW)
        .unwrap();
    let github = v.commit_write(staged, 2).unwrap().unwrap().id;
    (v, card, github)
}

fn values(
    v: &VaultService,
    id: &Uuid,
    top: &str,
    frames: &[(&str, &[CardRole])],
) -> Result<Vec<Vec<(CardRole, String)>>, Error> {
    let frames: Vec<CardFrame<'_>> = frames
        .iter()
        .map(|(url, roles)| CardFrame { url, roles })
        .collect();
    v.card_values_for_page(id, top, &frames).map(|all| {
        all.into_iter()
            .map(|f| f.into_iter().map(|(r, s)| (r, s.expose().to_owned())).collect())
            .collect()
    })
}

#[test]
fn https_pages_get_the_cards_overview_and_no_secrets() {
    let (v, card, _) = with_card();
    let list = v.cards_for_page(SHOP, None).unwrap();
    assert!(!list.insecure);
    assert_eq!(list.cards.len(), 1, "only cards, never logins");
    let c = &list.cards[0];
    assert_eq!(c.id, card);
    assert_eq!(c.title, "Visa pessoal");
    assert_eq!(c.last4.as_deref(), Some("1111"));
    assert_eq!(c.expiry.map(|e| (e.year, e.month)), Some((2033, 4)));
    assert!(!format!("{c:?}").contains("1111"));
}

#[test]
fn http_pages_are_told_they_are_insecure_and_get_nothing() {
    let (v, card, _) = with_card();
    let list = v.cards_for_page("http://shop.example.com/", None).unwrap();
    assert!(list.insecure);
    assert!(list.cards.is_empty());
    let r = values(&v, &card, "http://shop.example.com/", &[("http://shop.example.com/", &[CardRole::Number])]);
    assert!(matches!(r, Err(Error::Denied)));
}

#[test]
fn processor_and_same_site_frames_are_served() {
    let (v, card, _) = with_card();
    for frame in [
        "https://js.stripe.com/v3/elements-inner-card.html",
        "https://b.js.stripe.com/v3/fingerprinted/x.html",
        "https://checkoutshopper-live.adyen.com/checkoutshopper/securedfields/x/securedFields.html",
        "https://assets.braintreegateway.com/web/3.0/html/hosted-fields-frame.min.html",
        "https://secure-fields.mercadopago.com/",
        "https://pay.example.com/card",
    ] {
        assert!(!v.cards_for_page(frame, Some(SHOP)).unwrap().insecure, "{frame}");
    }
}

#[test]
fn look_alike_and_cross_site_frames_are_denied() {
    let (v, _, _) = with_card();
    for frame in [
        "https://js.stripe.com.evil.com/",
        "https://evil-js.stripe.com/",
        "https://evil.com/card",
        "http://js.stripe.com/v3/",
        "https://js.stripe.com:8443/",
    ] {
        assert!(matches!(v.cards_for_page(frame, Some(SHOP)), Err(Error::Denied)), "{frame}");
    }
}

#[test]
fn values_come_per_frame_as_asked_and_derived_in_rust() {
    let (v, card, _) = with_card();
    let got = values(
        &v,
        &card,
        SHOP,
        &[
            (SHOP, &[CardRole::CardholderName, CardRole::CardholderGivenName, CardRole::CardholderFamilyName]),
            (
                "https://js.stripe.com/v3/elements-inner-card.html",
                &[CardRole::Number, CardRole::ExpiryMonth, CardRole::ExpiryYear, CardRole::VerificationNumber, CardRole::Brand],
            ),
        ],
    )
    .unwrap();
    assert_eq!(
        got[0],
        vec![
            (CardRole::CardholderName, "Samuel S Rocha".to_owned()),
            (CardRole::CardholderGivenName, "Samuel".to_owned()),
            (CardRole::CardholderFamilyName, "S Rocha".to_owned()),
        ]
    );
    assert_eq!(
        got[1],
        vec![
            (CardRole::Number, VISA.to_owned()),
            (CardRole::ExpiryMonth, "4".to_owned()),
            (CardRole::ExpiryYear, "2033".to_owned()),
            (CardRole::VerificationNumber, "123".to_owned()),
            (CardRole::Brand, "visa".to_owned()),
        ]
    );
}

#[test]
fn roles_without_a_value_are_left_out() {
    let (mut v, _, _) = with_card();
    let staged = v.stage_create(card_input("", "5555555555554444", None), NOW).unwrap();
    let no_cvv = v.commit_write(staged, 3).unwrap().unwrap().id;
    let got = values(&v, &no_cvv, SHOP, &[(SHOP, &[CardRole::VerificationNumber, CardRole::Number])]).unwrap();
    assert_eq!(got[0], vec![(CardRole::Number, "5555555555554444".to_owned())]);
}

#[test]
fn one_bad_frame_denies_the_whole_request() {
    let (v, card, _) = with_card();
    let r = values(&v, &card, SHOP, &[(SHOP, &[CardRole::Number]), ("https://ads.example.net/", &[CardRole::Number])]);
    assert!(matches!(r, Err(Error::Denied)));
}

/// Attack: evil.com frames shop.com's checkout to collect the card through the user's click.
#[test]
fn a_checkout_framed_by_another_site_gets_nothing() {
    let (v, card, _) = with_card();
    let r = values(&v, &card, "https://evil.com/", &[(SHOP, &[CardRole::Number])]);
    assert!(matches!(r, Err(Error::Denied)));
}

#[test]
fn a_login_and_a_missing_item_answer_the_same() {
    let (v, _, github) = with_card();
    let a = values(&v, &github, SHOP, &[(SHOP, &[CardRole::Number])]);
    let b = values(&v, &Uuid::new_v4(), SHOP, &[(SHOP, &[CardRole::Number])]);
    assert!(matches!(a, Err(Error::NotFound)));
    assert!(matches!(b, Err(Error::NotFound)));
}

#[test]
fn locked_vaults_answer_locked() {
    let (mut v, card, _) = with_card();
    v.lock();
    assert!(matches!(v.cards_for_page(SHOP, None), Err(Error::Locked)));
    assert!(matches!(values(&v, &card, SHOP, &[(SHOP, &[CardRole::Number])]), Err(Error::Locked)));
}

fn new_card(number: &str) -> NewCard<'static> {
    NewCard {
        title: None,
        cardholder_name: Some("Samuel Rocha"),
        number: secret(number),
        verification_number: Some(secret("123")),
        expiry: Some("2030-01"),
    }
}

#[test]
fn a_typed_card_is_saved_from_the_checkout() {
    let (mut v, _, _) = with_card();
    let staged = v.stage_save_card(SHOP, None, new_card("4000 0566 5566 5556"), NOW).unwrap();
    let id = staged.item_id;
    v.commit_write(staged.write, 4).unwrap();
    let list = v.cards_for_page(SHOP, None).unwrap();
    let saved = list.cards.iter().find(|c| c.id == id).unwrap();
    assert_eq!(saved.title, "Visa", "the brand names a card saved without a title");
    assert_eq!(saved.last4.as_deref(), Some("5556"));
    assert_eq!(saved.expiry.map(|e| (e.year, e.month)), Some((2030, 1)));
}

#[test]
fn saving_refuses_bad_numbers_and_foreign_or_insecure_frames() {
    let (v, _, _) = with_card();
    assert!(matches!(
        v.stage_save_card(SHOP, None, new_card("4111111111111112"), NOW),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        v.stage_save_card("http://shop.example.com/", None, new_card(VISA), NOW),
        Err(Error::Denied)
    ));
    // A processor's frame fills, but a save comes only from the shop's own pages.
    assert!(matches!(
        v.stage_save_card("https://js.stripe.com/v3/", Some(SHOP), new_card(VISA), NOW),
        Err(Error::Denied)
    ));
    assert!(matches!(
        v.stage_save_card(SHOP, Some("https://evil.com/"), new_card(VISA), NOW),
        Err(Error::Denied)
    ));
    let bad_expiry = NewCard { expiry: Some("2030-13"), ..new_card(VISA) };
    assert!(matches!(v.stage_save_card(SHOP, None, bad_expiry, NOW), Err(Error::InvalidInput(_))));
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p havenkeys-core --test card_page`
Expected: a compile error: `unresolved import havenkeys_core::card_page`.

- [ ] **Step 3: Implement** (`crates/havenkeys-core/src/card_page.rs`)

```rust
//! Cards for web pages (spec 2026-09-29-card-autofill §6.2, §6.3).
//!
//! Unlike a login, a card is not bound to a site: any https checkout may
//! ask. What protects it is here and in the extension: https only; a frame
//! only when it is the same site as the tab's page or one of the payment
//! processors' card-field origins below; only the roles asked for; and the
//! user's pick in the extension's own menu.

use crate::card::{check_digit_ok, clean_number, detect_brand, CardBrand, CardExpiry, CardFields, CardInput};
use crate::error::{Error, Result};
use crate::identity_page::truncate_bytes;
use crate::model::{clean_title, ItemInput, ItemType, SecretUpdate};
use crate::origin::{host_key, site_of, PageUrl};
use crate::secret::SecretString;
use crate::vault::{StagedSave, VaultService};
use std::fmt;
use uuid::Uuid;

/// Exact origins payment processors serve card-field iframes from. Adding
/// one is a code change with a test (tests/card_page.rs).
pub const PAYMENT_FRAME_ORIGINS: &[&str] = &[
    // Stripe Elements. CSP frame-src for Stripe.js:
    // https://docs.stripe.com/security/guide
    "https://js.stripe.com",
    // Adyen Web card fields: securedFields.html loads from the client key's
    // live API environment. Official SDK source, github.com/Adyen/adyen-web:
    // packages/lib/src/core/Environment/constants.ts (API_ENVIRONMENTS) and
    // packages/lib/src/components/internal/SecuredFields/lib/CSF/extensions/handleConfig.ts (iframeSrc).
    "https://checkoutshopper-live.adyen.com",
    "https://checkoutshopper-live-us.adyen.com",
    "https://checkoutshopper-live-au.adyen.com",
    "https://checkoutshopper-live-apse.adyen.com",
    "https://checkoutshopper-live-in.adyen.com",
    "https://checkoutshopper-live-nea.adyen.com",
    // Braintree Hosted Fields. CSP frame-src (production):
    // https://braintree.github.io/braintree-web/current/
    "https://assets.braintreegateway.com",
    // Mercado Pago Secure Fields. SDK JS v2 (https://sdk.mercadopago.com/js/v2),
    // "prod" environment: the iframe loads cacheUrl, falling back to sourceUrl.
    "https://secure-fields.mercadopago.com",
    "https://api-static.mercadopago.com",
];

/// Hosts whose subdomains serve card fields too. Stripe: "Adding
/// `*.js.stripe.com` lets Stripe.js improve performance by starting frames
/// on different origins" (https://docs.stripe.com/security/guide).
pub const PAYMENT_FRAME_PARENTS: &[&str] = &["js.stripe.com"];

/// A frame a payment processor serves card fields from: https, default
/// port, an exact origin above or a subdomain of a parent above.
pub fn is_payment_frame(page: &PageUrl) -> bool {
    let url = page.url();
    if url.scheme() != "https" || url.port().is_some() {
        return false;
    }
    let origin = url.origin().ascii_serialization();
    if PAYMENT_FRAME_ORIGINS.contains(&origin.as_str()) {
        return true;
    }
    let Some(host) = host_key(url) else {
        return false;
    };
    PAYMENT_FRAME_PARENTS.iter().any(|parent| {
        host.len() > parent.len() + 1
            && host.ends_with(parent)
            && host.as_bytes()[host.len() - parent.len() - 1] == b'.'
    })
}

fn is_https(page: &PageUrl) -> bool {
    page.url().scheme() == "https"
}

fn same_site(a: &PageUrl, b: &PageUrl) -> bool {
    matches!((site_of(a), site_of(b)), (Some(x), Some(y)) if x == y)
}

/// May `frame`, in a tab showing `top`, be served a card?
fn frame_allowed(frame: &PageUrl, top: &PageUrl) -> bool {
    is_https(frame) && (same_site(frame, top) || is_payment_frame(frame))
}

/// What a checkout field asks for (spec §4). The extension shapes the value
/// to the field; nothing here depends on the field's layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardRole {
    CardholderName,
    CardholderGivenName,
    CardholderFamilyName,
    Number,
    VerificationNumber,
    ExpiryMonth,
    ExpiryYear,
    Brand,
}

/// One card offered to a page: overview data only.
pub struct CardOffer {
    pub id: Uuid,
    pub title: String,
    pub brand: Option<CardBrand>,
    pub last4: Option<String>,
    pub expiry: Option<CardExpiry>,
}

impl fmt::Debug for CardOffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CardOffer").field("id", &self.id).finish_non_exhaustive()
    }
}

pub struct CardList {
    /// The tab's page is not https: no card is offered there.
    pub insecure: bool,
    pub cards: Vec<CardOffer>,
}

/// One frame of a fill: its URL (from the browser) and the roles its fields ask for.
pub struct CardFrame<'a> {
    pub url: &'a str,
    pub roles: &'a [CardRole],
}

/// A card the user typed into a checkout and confirmed in the save prompt.
pub struct NewCard<'a> {
    pub title: Option<&'a str>,
    pub cardholder_name: Option<&'a str>,
    pub number: SecretString,
    pub verification_number: Option<SecretString>,
    pub expiry: Option<&'a str>,
}

/// Cards offered to one page, at most.
pub const MAX_PAGE_CARDS: usize = 50;

/// A role's value, or `None` when the card has none.
fn card_value(f: &CardFields, role: CardRole) -> Option<SecretString> {
    let words: Option<Vec<&str>> = f
        .cardholder_name
        .as_ref()
        .map(|n| n.expose().split_whitespace().collect());
    let v = match role {
        CardRole::CardholderName => words.map(|w| w.join(" ")),
        CardRole::CardholderGivenName => words.and_then(|w| w.first().map(|s| (*s).to_owned())),
        CardRole::CardholderFamilyName => words.filter(|w| w.len() > 1).map(|w| w[1..].join(" ")),
        CardRole::Number => f.number.as_ref().map(|n| n.expose().to_owned()),
        CardRole::VerificationNumber => f.verification_number.as_ref().map(|n| n.expose().to_owned()),
        CardRole::ExpiryMonth => f.expiry.map(|e| e.month.to_string()),
        CardRole::ExpiryYear => f.expiry.map(|e| format!("{:04}", e.year)),
        // The user's choice, else detected from the number, else "other".
        CardRole::Brand => Some(f.effective_brand().id().to_owned()),
    }?;
    (!v.is_empty()).then(|| SecretString::new(v))
}

impl VaultService {
    /// The cards a page may be offered. `insecure` for a non-https tab.
    pub fn cards_for_page(&self, page_url: &str, top_url: Option<&str>) -> Result<CardList> {
        self.session()?;
        let page = PageUrl::parse(page_url).ok_or(Error::Denied)?;
        let top = PageUrl::parse(top_url.unwrap_or(page_url)).ok_or(Error::Denied)?;
        if !is_https(&top) {
            return Ok(CardList { insecure: true, cards: Vec::new() });
        }
        if top_url.is_some() && !frame_allowed(&page, &top) {
            return Err(Error::Denied);
        }
        let cards = self
            .list_items()?
            .iter()
            .filter(|o| o.item_type == ItemType::Card)
            .take(MAX_PAGE_CARDS)
            .map(|o| {
                let summary = o.card.clone();
                CardOffer {
                    id: o.id,
                    title: truncate_bytes(o.title.clone(), crate::identity_page::MAX_SUMMARY_TITLE_BYTES),
                    brand: summary.as_ref().map(|s| s.brand),
                    last4: summary.as_ref().and_then(|s| s.last4.clone()),
                    expiry: summary.and_then(|s| s.expiry),
                }
            })
            .collect();
        Ok(CardList { insecure: false, cards })
    }

    /// One card's values for each frame, in order. Every frame must be
    /// allowed, or nothing is returned.
    pub fn card_values_for_page(
        &self,
        item_id: &Uuid,
        top_url: &str,
        frames: &[CardFrame<'_>],
    ) -> Result<Vec<Vec<(CardRole, SecretString)>>> {
        self.session()?;
        let top = PageUrl::parse(top_url).ok_or(Error::Denied)?;
        if !is_https(&top) {
            return Err(Error::Denied);
        }
        for frame in frames {
            let page = PageUrl::parse(frame.url).ok_or(Error::Denied)?;
            if !frame_allowed(&page, &top) {
                return Err(Error::Denied);
            }
        }
        // A login, a note or a missing item: one answer, so a page cannot
        // learn which IDs exist.
        let fields = self.card_fields(*item_id).map_err(|e| match e {
            Error::Denied => Error::NotFound,
            other => other,
        })?;
        Ok(frames
            .iter()
            .map(|frame| {
                frame
                    .roles
                    .iter()
                    .filter_map(|r| card_value(&fields, *r).map(|v| (*r, v)))
                    .collect()
            })
            .collect())
    }

    /// Stage a new card typed into a checkout. The frame must be the shop's
    /// own (https, same site as the tab), never a processor's.
    pub fn stage_save_card(
        &self,
        page_url: &str,
        top_url: Option<&str>,
        card: NewCard<'_>,
        now_ms: i64,
    ) -> Result<StagedSave> {
        self.session()?;
        let page = PageUrl::parse(page_url).ok_or(Error::Denied)?;
        if !is_https(&page) {
            return Err(Error::Denied);
        }
        if let Some(top) = top_url {
            let top = PageUrl::parse(top).ok_or(Error::Denied)?;
            if !is_https(&top) || !same_site(&page, &top) {
                return Err(Error::Denied);
            }
        }
        let digits = clean_number(card.number.expose())?;
        if !check_digit_ok(&digits) {
            return Err(Error::InvalidInput("the card number fails its check digit"));
        }
        let expiry = card.expiry.map(CardExpiry::parse).transpose()?;
        let title = match card.title {
            Some(t) => clean_title(t)?,
            None => detect_brand(&digits).map_or("Card", |b| b.display_name()).to_owned(),
        };
        let input = ItemInput {
            item_type: ItemType::Card,
            title,
            username: None,
            urls: vec![],
            password: SecretUpdate::Keep,
            totp: SecretUpdate::Keep,
            notes: SecretUpdate::Keep,
            content: SecretUpdate::Keep,
            auto_sign_in: None,
            sign_in_with: None,
            identity: None,
            card: Some(CardInput {
                cardholder_name: card.cardholder_name.map(SecretString::from),
                brand: None,
                number: SecretUpdate::Set(SecretString::new(digits)),
                verification_number: card.verification_number.map_or(SecretUpdate::Keep, SecretUpdate::Set),
                expiry,
                notes: None,
            }),
        };
        let write = self.stage_create(input, now_ms)?;
        Ok(StagedSave {
            item_id: write.item_id,
            write,
        })
    }
}
```

In `crates/havenkeys-core/src/lib.rs`, add `pub mod card_page;` next to `pub mod card;`. In `identity_page.rs`, make `truncate_bytes` `pub(crate)`. If `host_key` in `origin.rs` is not reachable from `card_page.rs`, it already is: it is `pub(crate)`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p havenkeys-core --test card_page && cargo test -p havenkeys-core --lib identity_page`
Expected: all pass.

- [ ] **Step 5: Lint**

Run: `cargo clippy -p havenkeys-core --all-targets -- -D warnings && cargo fmt --all --check`
Expected: clean. Run `cargo fmt --all` if needed.

- [ ] **Step 6: Commit**

```bash
git add crates/havenkeys-core/src/card_page.rs crates/havenkeys-core/src/lib.rs crates/havenkeys-core/src/identity_page.rs crates/havenkeys-core/tests/card_page.rs
git commit -m "feat(core): cards for web pages: https, same-site or processor frames, roles"
```

---

### Task 2: Protocol (Rust) — card requests and results

**Files:**
- Modify: `crates/havenkeys-protocol/src/lib.rs` (constants after `MAX_IDENTITY_VALUE_BYTES`)
- Modify: `crates/havenkeys-protocol/src/message.rs`
- Test: `crates/havenkeys-protocol/tests/messages.rs`

**Interfaces:**
- Produces:
  - `MAX_CARD_FRAMES = 8`, `MAX_CARD_ROLES = 8`, `MAX_CARD_VALUE_BYTES = 1024`, `MAX_CARD_NUMBER_BYTES = 64`, `MAX_CARD_CODE_BYTES = 16`.
  - `enum CardRole` (wire camelCase) and `enum CardBrandId` (wire lowercase).
  - `struct CardFrame { url, roles }`, `struct CardMatch { id, title, brand: Option<CardBrandId>, last4: Option<String>, expiry: Option<String> }`, `struct CardValue { role, value: WireSecret }`, `struct CardFrameValues { values }`.
  - `Request::{FindCards { url, top_url }, FillCard { item_id, top_url: String, frames }, SaveCard { url, top_url, title, cardholder_name, number: WireSecret, verification_number: Option<WireSecret>, expiry: Option<String> }}`.
  - `ResultBody::{FindCards { insecure, cards }, FillCard { frames }, SaveCard { item_id }}`.

- [ ] **Step 1: Write the failing tests** (append to `tests/messages.rs`)

```rust
#[test]
fn card_requests_parse_and_are_bounded() {
    let ok = [
        r#"{"v":1,"id":1,"request":{"type":"find_cards","url":"https://shop.com/"}}"#.to_owned(),
        format!(
            r#"{{"v":1,"id":2,"request":{{"type":"fill_card","itemId":"{ITEM}","topUrl":"https://shop.com/","frames":[{{"url":"https://shop.com/","roles":["cardholderName"]}},{{"url":"https://js.stripe.com/v3/","roles":["number","expiryMonth","expiryYear","verificationNumber"]}}]}}}}"#
        ),
        r#"{"v":1,"id":3,"request":{"type":"save_card","url":"https://shop.com/","number":"4111 1111 1111 1111","expiry":"2030-01"}}"#.to_owned(),
        r#"{"v":1,"id":4,"request":{"type":"save_card","url":"https://shop.com/","title":"Visa","cardholderName":"Samuel","number":"4111111111111111","verificationNumber":"123","expiry":"2030-01"}}"#.to_owned(),
    ];
    for s in &ok {
        let env = parse(s).unwrap_or_else(|_| panic!("{s}"));
        assert!(matches!(env.request.kind(), "find_cards" | "fill_card" | "save_card"));
    }
    let frame = r#"{"url":"https://shop.com/","roles":["number"]}"#;
    let nine = vec![frame; 9].join(",");
    let bad = [
        format!(r#"{{"v":1,"id":5,"request":{{"type":"fill_card","itemId":"{ITEM}","topUrl":"https://shop.com/","frames":[]}}}}"#),
        format!(r#"{{"v":1,"id":6,"request":{{"type":"fill_card","itemId":"{ITEM}","topUrl":"https://shop.com/","frames":[{nine}]}}}}"#),
        format!(r#"{{"v":1,"id":7,"request":{{"type":"fill_card","itemId":"{ITEM}","topUrl":"https://shop.com/","frames":[{{"url":"https://shop.com/","roles":[]}}]}}}}"#),
        format!(r#"{{"v":1,"id":8,"request":{{"type":"fill_card","itemId":"{ITEM}","topUrl":"https://shop.com/","frames":[{{"url":"https://shop.com/","roles":["number","number"]}}]}}}}"#),
        format!(r#"{{"v":1,"id":9,"request":{{"type":"fill_card","itemId":"{ITEM}","topUrl":"https://shop.com/","frames":[{{"url":"https://shop.com/","roles":["password"]}}]}}}}"#),
        format!(r#"{{"v":1,"id":10,"request":{{"type":"fill_card","itemId":"{ITEM}","topUrl":"https://shop.com/","frames":[{{"url":"","roles":["number"]}}]}}}}"#),
        format!(r#"{{"v":1,"id":11,"request":{{"type":"fill_card","itemId":"{ITEM}","topUrl":"https://shop.com/","frames":[{{"url":"https://shop.com/","roles":["number"],"x":1}}]}}}}"#),
        r#"{"v":1,"id":12,"request":{"type":"save_card","url":"https://shop.com/","number":""}}"#.to_owned(),
        r#"{"v":1,"id":13,"request":{"type":"save_card","url":"https://shop.com/","number":"4111111111111111","expiry":"01/2030"}}"#.to_owned(),
        r#"{"v":1,"id":14,"request":{"type":"save_card","url":"https://shop.com/","number":"4111111111111111","verificationNumber":"12345678901234567"}}"#.to_owned(),
        format!(r#"{{"v":1,"id":15,"request":{{"type":"save_card","url":"https://shop.com/","number":"{}"}}}}"#, "4".repeat(65)),
        r#"{"v":1,"id":16,"request":{"type":"save_card","url":"https://shop.com/","number":"4111111111111111","title":""}}"#.to_owned(),
        r#"{"v":1,"id":17,"request":{"type":"find_cards","url":"https://shop.com/","itemId":"x"}}"#.to_owned(),
    ];
    for s in &bad {
        assert!(parse(s).is_err(), "{s}");
    }
}

#[test]
fn card_results_are_validated() {
    let valid = [
        format!(r#"{{"v":1,"id":1,"result":{{"type":"find_cards","insecure":false,"cards":[{{"id":"{ITEM}","title":"Visa","brand":"visa","last4":"1111","expiry":"2033-04"}}]}}}}"#),
        r#"{"v":1,"id":2,"result":{"type":"find_cards","insecure":true,"cards":[]}}"#.to_owned(),
        r#"{"v":1,"id":3,"result":{"type":"fill_card","frames":[{"values":[{"role":"number","value":"4111111111111111"}]},{"values":[]}]}}"#.to_owned(),
        format!(r#"{{"v":1,"id":4,"result":{{"type":"save_card","itemId":"{ITEM}"}}}}"#),
        format!(r#"{{"v":1,"id":5,"result":{{"type":"find_cards","insecure":false,"cards":[{{"id":"{ITEM}","title":"x","brand":null,"last4":null,"expiry":null}}]}}}}"#),
    ];
    for s in &valid {
        assert!(Outgoing::parse(s.as_bytes()).is_some(), "{s}");
    }
    let long = "x".repeat(MAX_CARD_VALUE_BYTES + 1);
    let invalid = [
        format!(r#"{{"v":1,"id":1,"result":{{"type":"find_cards","insecure":true,"cards":[{{"id":"{ITEM}","title":"x","brand":null,"last4":null,"expiry":null}}]}}}}"#),
        format!(r#"{{"v":1,"id":1,"result":{{"type":"find_cards","insecure":false,"cards":[{{"id":"{ITEM}","title":"x","brand":"visa","last4":"41111","expiry":null}}]}}}}"#),
        format!(r#"{{"v":1,"id":1,"result":{{"type":"find_cards","insecure":false,"cards":[{{"id":"{ITEM}","title":"x","brand":"visa","last4":null,"expiry":"04/2033"}}]}}}}"#),
        format!(r#"{{"v":1,"id":1,"result":{{"type":"find_cards","insecure":false,"cards":[{{"id":"{ITEM}","title":"x","brand":"nubank","last4":null,"expiry":null}}]}}}}"#),
        r#"{"v":1,"id":1,"result":{"type":"fill_card","frames":[{"values":[{"role":"number","value":""}]}]}}"#.to_owned(),
        r#"{"v":1,"id":1,"result":{"type":"fill_card","frames":[{"values":[{"role":"number","value":"1"},{"role":"number","value":"2"}]}]}}"#.to_owned(),
        format!(r#"{{"v":1,"id":1,"result":{{"type":"fill_card","frames":[{{"values":[{{"role":"number","value":"{long}"}}]}}]}}}}"#),
        format!(r#"{{"v":1,"id":1,"result":{{"type":"fill_card","frames":[{}]}}}}"#, vec![r#"{"values":[]}"#; 9].join(",")),
    ];
    for s in &invalid {
        assert!(Outgoing::parse(s.as_bytes()).is_none(), "{s}");
    }
}

#[test]
fn card_values_and_requests_never_debug_print() {
    let v = CardValue {
        role: CardRole::Number,
        value: WireSecret::new("4111111111111111".into()),
    };
    assert!(!format!("{v:?}").contains("4111"));
    let env = parse(r#"{"v":1,"id":1,"request":{"type":"save_card","url":"https://shop.com/","number":"4111111111111111"}}"#).unwrap();
    assert!(!format!("{env:?}").contains("4111"));
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p havenkeys-protocol --test messages card_`
Expected: a compile error: `CardValue` not found.

- [ ] **Step 3: Implement**

In `crates/havenkeys-protocol/src/lib.rs`, after `MAX_IDENTITY_VALUE_BYTES`:

```rust
/// Frames one fill_card may cover (Stripe puts each card field in its own frame).
pub const MAX_CARD_FRAMES: usize = 8;
/// Roles one frame of a fill_card may ask for (there are eight).
pub const MAX_CARD_ROLES: usize = 8;
/// Largest card value returned (a cardholder name of 256 characters).
pub const MAX_CARD_VALUE_BYTES: usize = 4 * 256;
/// Largest card number accepted in save_card, spaces and dashes included.
pub const MAX_CARD_NUMBER_BYTES: usize = 64;
/// Largest verification number accepted in save_card.
pub const MAX_CARD_CODE_BYTES: usize = 16;
```

In `message.rs`:

1. Add `MAX_CARD_CODE_BYTES, MAX_CARD_FRAMES, MAX_CARD_NUMBER_BYTES, MAX_CARD_ROLES, MAX_CARD_VALUE_BYTES` to the `use crate::{…}` list.
2. Add to `enum Request`, after `OpenIdentity`:

```rust
    /// Cards any https page may be offered: overview data only, never a
    /// number or code. `insecure`: the tab's page is not https.
    FindCards {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
    },
    /// One card's values for the tab's card fields, one entry per frame.
    /// Every frame must be https and same-site as `top_url`, or a payment
    /// processor's (the core decides); one bad frame denies the request.
    FillCard {
        item_id: Uuid,
        top_url: String,
        frames: Vec<CardFrame>,
    },
    /// Save a card typed into a checkout after the user confirmed it. A server write.
    SaveCard {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        top_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cardholder_name: Option<String>,
        number: WireSecret,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        verification_number: Option<WireSecret>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expiry: Option<String>,
    },
```

3. After `roles_unique`, add the card types and helpers:

```rust
/// A checkout field's role for a card fill. Mirrors
/// havenkeys_core::card_page::CardRole; the bridge maps between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardRole {
    CardholderName,
    CardholderGivenName,
    CardholderFamilyName,
    Number,
    VerificationNumber,
    ExpiryMonth,
    ExpiryYear,
    Brand,
}

/// A card network. Mirrors havenkeys_core::card::CardBrand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CardBrandId {
    Visa,
    Mastercard,
    Amex,
    Elo,
    Hipercard,
    Diners,
    Discover,
    Jcb,
    Unionpay,
    Maestro,
    Other,
}

/// One frame of a fill_card: its URL (from the browser) and its fields' roles.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardFrame {
    pub url: String,
    pub roles: Vec<CardRole>,
}

/// A card offered to a page. No number beyond the last four digits.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardMatch {
    pub id: Uuid,
    pub title: String,
    #[serde(deserialize_with = "required")]
    pub brand: Option<CardBrandId>,
    #[serde(deserialize_with = "required")]
    pub last4: Option<String>,
    /// `YYYY-MM`.
    #[serde(deserialize_with = "required")]
    pub expiry: Option<String>,
}

impl fmt::Debug for CardMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CardMatch").field("id", &self.id).finish_non_exhaustive()
    }
}

/// One card value for a fill.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardValue {
    pub role: CardRole,
    pub value: WireSecret,
}

impl fmt::Debug for CardValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CardValue").field("role", &self.role).finish_non_exhaustive()
    }
}

/// The values for one frame of a fill_card.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardFrameValues {
    pub values: Vec<CardValue>,
}

fn card_roles_unique(roles: &[CardRole]) -> bool {
    let mut seen = HashSet::new();
    roles.iter().all(|r| seen.insert(*r))
}

/// `YYYY-MM` in shape (the core checks the month).
fn expiry_shape_ok(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7 && b[4] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || c.is_ascii_digit())
}
```

4. In `kind()`, add `Request::FindCards { .. } => "find_cards"`, `Request::FillCard { .. } => "fill_card"` and `Request::SaveCard { .. } => "save_card"`.
5. Replace `urls()` and the first lines of `field_sizes_ok()`, because a fill_card carries several URLs:

```rust
    fn urls(&self) -> Vec<&str> {
        match self {
            Request::Status {} | Request::Lock {} | Request::GeneratePassword {} => Vec::new(),
            Request::FillCard { top_url, frames, .. } => std::iter::once(top_url.as_str())
                .chain(frames.iter().map(|f| f.url.as_str()))
                .collect(),
            Request::FindMatches { url, top_url }
            | Request::FillItem { url, top_url, .. }
            | Request::GetTotp { url, top_url, .. }
            | Request::CheckLogin { url, top_url, .. }
            | Request::SaveLogin { url, top_url, .. }
            | Request::FindPasskeys { url, top_url, .. }
            | Request::PasskeyGet { url, top_url, .. }
            | Request::CheckPasskeyCreate { url, top_url, .. }
            | Request::PasskeyCreate { url, top_url, .. }
            | Request::PasskeyStatus { url, top_url }
            | Request::OpenItem { url, top_url, .. }
            | Request::StartSso { url, top_url, .. }
            | Request::CheckSso { url, top_url, .. }
            | Request::SaveSso { url, top_url, .. }
            | Request::FindIdentity { url, top_url }
            | Request::FillIdentity { url, top_url, .. }
            | Request::OpenIdentity { url, top_url }
            | Request::FindCards { url, top_url }
            | Request::SaveCard { url, top_url, .. } => [Some(url.as_str()), top_url.as_deref()]
                .into_iter()
                .flatten()
                .collect(),
        }
    }

    fn field_sizes_ok(&self) -> bool {
        let urls_ok = self
            .urls()
            .into_iter()
            .all(|u| !u.is_empty() && u.len() <= MAX_URL_BYTES);
```

(The rest of `field_sizes_ok` stays; add `card_ok` before its last line.)

```rust
        let card_ok = match self {
            Request::FillCard { frames, .. } => {
                !frames.is_empty()
                    && frames.len() <= MAX_CARD_FRAMES
                    && frames.iter().all(|f| {
                        !f.roles.is_empty() && f.roles.len() <= MAX_CARD_ROLES && card_roles_unique(&f.roles)
                    })
            }
            Request::SaveCard {
                title,
                cardholder_name,
                number,
                verification_number,
                expiry,
                ..
            } => {
                title_ok(title, &None)
                    && !number.expose().is_empty()
                    && number.expose().len() <= MAX_CARD_NUMBER_BYTES
                    && verification_number
                        .as_ref()
                        .is_none_or(|c| !c.expose().is_empty() && c.expose().len() <= MAX_CARD_CODE_BYTES)
                    && cardholder_name
                        .as_ref()
                        .is_none_or(|n| !n.is_empty() && n.len() <= MAX_CARD_VALUE_BYTES)
                    && expiry.as_deref().is_none_or(expiry_shape_ok)
            }
            _ => true,
        };
        urls_ok && login_ok && passkey_ok && identity_ok && card_ok
    }
```

6. In `Response::is_valid`, before `_ => true`:

```rust
            Some(ResultBody::FindCards { insecure, cards }) => {
                cards.len() <= MAX_MATCHES
                    && !(*insecure && !cards.is_empty())
                    && cards.iter().all(|c| {
                        c.title.len() <= MAX_TITLE_BYTES
                            && c.last4
                                .as_deref()
                                .is_none_or(|l| l.len() == 4 && l.bytes().all(|b| b.is_ascii_digit()))
                            && c.expiry.as_deref().is_none_or(expiry_shape_ok)
                    })
            }
            Some(ResultBody::FillCard { frames }) => {
                frames.len() <= MAX_CARD_FRAMES
                    && frames.iter().all(|f| {
                        let roles: Vec<CardRole> = f.values.iter().map(|v| v.role).collect();
                        f.values.len() <= MAX_CARD_ROLES
                            && card_roles_unique(&roles)
                            && f.values.iter().all(|v| {
                                !v.value.expose().is_empty() && v.value.expose().len() <= MAX_CARD_VALUE_BYTES
                            })
                    })
            }
```

7. In `enum ResultBody`, after `OpenIdentity {}`:

```rust
    FindCards {
        insecure: bool,
        cards: Vec<CardMatch>,
    },
    FillCard {
        frames: Vec<CardFrameValues>,
    },
    SaveCard {
        item_id: Uuid,
    },
```

8. In `ResultBody`'s `Debug`, add `ResultBody::FindCards { .. } => "find_cards"`, `ResultBody::FillCard { .. } => "fill_card"` and `ResultBody::SaveCard { .. } => "save_card"`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p havenkeys-protocol`
Expected: all pass, including the existing fuzz loop.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p havenkeys-protocol --all-targets -- -D warnings && cargo fmt --all --check
git add crates/havenkeys-protocol
git commit -m "feat(protocol): find_cards, fill_card and save_card"
```

---

### Task 3: Bridge — dispatch, rate limits, security tests

**Files:**
- Modify: `crates/havenkeys-bridge/src/dispatch.rs`
- Modify: `crates/havenkeys-bridge/src/server.rs`
- Modify: `crates/havenkeys-bridge/src/ratelimit.rs` (doc comment of `RequestClass` only)
- Test: `crates/havenkeys-bridge/tests/bridge.rs`

**Interfaces:**
- Consumes: Task 1 (`cards_for_page`, `card_values_for_page`, `stage_save_card`, `CardFrame`, `CardRole`, `NewCard`) and Task 2 (the wire types).
- Produces: `Dispatched::SaveCard(StagedSave)`. Answers the three requests.

- [ ] **Step 1: Write the failing tests** (append to `tests/bridge.rs`)

```rust
use havenkeys_core::card::{CardExpiry, CardInput};

/// Give the fixture's vault a Visa with a CVV.
fn add_card(f: &Fixture) -> Uuid {
    let mut v = f.vault.lock().unwrap();
    let input = ItemInput {
        item_type: ItemType::Card,
        title: "Visa".into(),
        username: None,
        urls: vec![],
        password: SecretUpdate::Keep,
        totp: SecretUpdate::Keep,
        notes: SecretUpdate::Keep,
        content: SecretUpdate::Keep,
        auto_sign_in: None,
        sign_in_with: None,
        identity: None,
        card: Some(CardInput {
            cardholder_name: Some(SecretString::from("Samuel Rocha")),
            brand: None,
            number: SecretUpdate::Set(SecretString::from("4111111111111111")),
            verification_number: SecretUpdate::Set(SecretString::from("123")),
            expiry: Some(CardExpiry { year: 2033, month: 11 }),
            notes: None,
        }),
    };
    let staged = v.stage_create(input, NOW).unwrap();
    v.commit_write(staged, 60).unwrap().unwrap().id
}

fn fill_card(f: &Fixture, id: Uuid, top: &str, frames: serde_json::Value) -> serde_json::Value {
    call(
        f,
        serde_json::json!({"type": "fill_card", "itemId": id, "topUrl": top, "frames": frames}),
    )
}

#[test]
fn cards_are_found_and_filled_across_processor_frames() {
    let f = fixture();
    let id = add_card(&f);
    let found = call(&f, serde_json::json!({"type": "find_cards", "url": "https://shop.com/checkout"}));
    assert_eq!(found["result"]["insecure"], false);
    assert_eq!(
        found["result"]["cards"],
        serde_json::json!([{"id": id, "title": "Visa", "brand": "visa", "last4": "1111", "expiry": "2033-11"}])
    );
    assert!(!found.to_string().contains("4111111111111111"));
    let r = fill_card(
        &f,
        id,
        "https://shop.com/checkout",
        serde_json::json!([
            {"url": "https://shop.com/checkout", "roles": ["cardholderName"]},
            {"url": "https://js.stripe.com/v3/elements-inner-card.html", "roles": ["number", "expiryMonth", "expiryYear", "verificationNumber"]}
        ]),
    );
    assert_eq!(
        r["result"]["frames"],
        serde_json::json!([
            {"values": [{"role": "cardholderName", "value": "Samuel Rocha"}]},
            {"values": [
                {"role": "number", "value": "4111111111111111"},
                {"role": "expiryMonth", "value": "11"},
                {"role": "expiryYear", "value": "2033"},
                {"role": "verificationNumber", "value": "123"}
            ]}
        ])
    );
}

/// Attacks: an http checkout, a look-alike processor host, and evil.com
/// framing a real checkout.
#[test]
fn cards_are_denied_to_insecure_and_foreign_frames() {
    let f = fixture();
    let id = add_card(&f);
    let http = call(&f, serde_json::json!({"type": "find_cards", "url": "http://shop.com/"}));
    assert_eq!(http["result"], serde_json::json!({"type": "find_cards", "insecure": true, "cards": []}));
    let one = |url: &str| serde_json::json!([{"url": url, "roles": ["number"]}]);
    assert_eq!(error_code(&fill_card(&f, id, "http://shop.com/", one("http://shop.com/"))), Some("denied"));
    assert_eq!(
        error_code(&fill_card(&f, id, "https://shop.com/", one("https://js.stripe.com.evil.com/"))),
        Some("denied")
    );
    assert_eq!(error_code(&fill_card(&f, id, "https://evil.com/", one("https://shop.com/checkout"))), Some("denied"));
}

#[test]
fn fill_card_answers_not_found_for_logins_and_unknown_ids() {
    let f = fixture();
    add_card(&f);
    let frames = serde_json::json!([{"url": "https://shop.com/", "roles": ["number"]}]);
    assert_eq!(error_code(&fill_card(&f, f.github, "https://shop.com/", frames.clone())), Some("not_found"));
    assert_eq!(error_code(&fill_card(&f, Uuid::new_v4(), "https://shop.com/", frames)), Some("not_found"));
}

/// Regression: login requests never hand out a card.
#[test]
fn login_requests_never_return_a_card() {
    let f = fixture();
    let id = add_card(&f);
    assert_eq!(error_code(&fill(&f, id, "https://shop.com/")), Some("denied"));
    assert_eq!(error_code(&totp(&f, id, "https://shop.com/")), Some("denied"));
    let m = find(&f, "https://shop.com/");
    assert_eq!(m["result"]["matches"], serde_json::json!([]));
}

#[test]
fn save_card_writes_through_the_server_and_needs_it() {
    let offline = fixture();
    let req = serde_json::json!({"type": "save_card", "url": "https://shop.com/", "number": "4000 0566 5566 5556", "expiry": "2030-01"});
    assert_eq!(error_code(&call(&offline, req.clone())), Some("offline"));

    let f = online_fixture();
    let before = f.changes.load(Ordering::SeqCst);
    let r = call(&f, req);
    let id = r["result"]["itemId"].as_str().expect("saved").to_owned();
    assert_eq!(f.changes.load(Ordering::SeqCst), before + 1);
    let found = call(&f, serde_json::json!({"type": "find_cards", "url": "https://shop.com/"}));
    assert!(found.to_string().contains(&id));

    let bad = call(&f, serde_json::json!({"type": "save_card", "url": "https://shop.com/", "number": "4111111111111112"}));
    assert_eq!(error_code(&bad), Some("invalid_input"));
}

#[test]
fn card_requests_respect_lock_integration_and_rate_limits() {
    let f = fixture();
    let id = add_card(&f);
    let frames = serde_json::json!([{"url": "https://shop.com/", "roles": ["number"]}]);
    for _ in 0..10 {
        fill_card(&f, id, "https://shop.com/", frames.clone());
    }
    assert_eq!(error_code(&fill(&f, f.github, "https://github.com/")), Some("rate_limited"));

    let g = fixture();
    add_card(&g);
    g.vault
        .lock()
        .unwrap()
        .update_settings(Settings { browser_integration: false, ..Settings::default() })
        .unwrap();
    let r = call(&g, serde_json::json!({"type": "find_cards", "url": "https://shop.com/"}));
    assert_eq!(error_code(&r), Some("integration_disabled"));
    g.vault.lock().unwrap().lock();
    let r = call(&g, serde_json::json!({"type": "find_cards", "url": "https://shop.com/"}));
    assert_eq!(error_code(&r), Some("locked"));
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p havenkeys-bridge --test bridge card`
Expected: the tests fail with `malformed`/`internal` codes, or `dispatch` fails to compile because the match is non-exhaustive.

- [ ] **Step 3: Implement `dispatch.rs`**

Imports: add `use havenkeys_core::card::{CardBrand, CardExpiry};` and `use havenkeys_core::card_page::{CardFrame as CoreCardFrame, CardRole as CoreCardRole, NewCard};`. Extend the protocol import with `CardBrandId, CardFrameValues, CardMatch, CardRole, CardValue`. Add `use havenkeys_core::vault::StagedSave;` if `StagedSave` is not already imported (it is used by `Dispatched::Save`).

Add the mapping helpers after `wire_role`:

```rust
fn core_card_role(r: CardRole) -> CoreCardRole {
    match r {
        CardRole::CardholderName => CoreCardRole::CardholderName,
        CardRole::CardholderGivenName => CoreCardRole::CardholderGivenName,
        CardRole::CardholderFamilyName => CoreCardRole::CardholderFamilyName,
        CardRole::Number => CoreCardRole::Number,
        CardRole::VerificationNumber => CoreCardRole::VerificationNumber,
        CardRole::ExpiryMonth => CoreCardRole::ExpiryMonth,
        CardRole::ExpiryYear => CoreCardRole::ExpiryYear,
        CardRole::Brand => CoreCardRole::Brand,
    }
}

fn wire_card_role(r: CoreCardRole) -> CardRole {
    match r {
        CoreCardRole::CardholderName => CardRole::CardholderName,
        CoreCardRole::CardholderGivenName => CardRole::CardholderGivenName,
        CoreCardRole::CardholderFamilyName => CardRole::CardholderFamilyName,
        CoreCardRole::Number => CardRole::Number,
        CoreCardRole::VerificationNumber => CardRole::VerificationNumber,
        CoreCardRole::ExpiryMonth => CardRole::ExpiryMonth,
        CoreCardRole::ExpiryYear => CardRole::ExpiryYear,
        CoreCardRole::Brand => CardRole::Brand,
    }
}

fn wire_brand(b: CardBrand) -> CardBrandId {
    match b {
        CardBrand::Visa => CardBrandId::Visa,
        CardBrand::Mastercard => CardBrandId::Mastercard,
        CardBrand::Amex => CardBrandId::Amex,
        CardBrand::Elo => CardBrandId::Elo,
        CardBrand::Hipercard => CardBrandId::Hipercard,
        CardBrand::Diners => CardBrandId::Diners,
        CardBrand::Discover => CardBrandId::Discover,
        CardBrand::Jcb => CardBrandId::Jcb,
        CardBrand::Unionpay => CardBrandId::Unionpay,
        CardBrand::Maestro => CardBrandId::Maestro,
        CardBrand::Other => CardBrandId::Other,
    }
}
```

Add to `enum Dispatched`:

```rust
    /// A card saved from a checkout. Same reason as `Save`: the write
    /// reaches the server before the item ID is returned.
    SaveCard(StagedSave),
```

Add the arms to `dispatch`, after `OpenIdentity`. `fill_card` maps errors with `code`, not `item_code`: the core already answers `NotFound` alike for a missing item and a non-card, and the extension shows "no longer in HavenKeys" for it.

```rust
        Request::FindCards { url, top_url } => {
            require_enabled(v)?;
            let list = v.cards_for_page(url, top_url.as_deref()).map_err(code)?;
            let cards = list
                .cards
                .into_iter()
                .take(MAX_MATCHES)
                .map(|c| CardMatch {
                    id: c.id,
                    title: c.title,
                    brand: c.brand.map(wire_brand),
                    last4: c.last4,
                    expiry: c.expiry.map(CardExpiry::to_wire),
                })
                .collect();
            Ok(Dispatched::Done(ResultBody::FindCards {
                insecure: list.insecure,
                cards,
            }))
        }
        Request::FillCard {
            item_id,
            top_url,
            frames,
        } => {
            require_enabled(v)?;
            let roles: Vec<Vec<CoreCardRole>> = frames
                .iter()
                .map(|f| f.roles.iter().copied().map(core_card_role).collect())
                .collect();
            let core_frames: Vec<CoreCardFrame<'_>> = frames
                .iter()
                .zip(&roles)
                .map(|(f, r)| CoreCardFrame { url: &f.url, roles: r })
                .collect();
            let values = v
                .card_values_for_page(item_id, top_url, &core_frames)
                .map_err(code)?
                .into_iter()
                .map(|frame| CardFrameValues {
                    values: frame
                        .into_iter()
                        .map(|(role, value)| CardValue {
                            role: wire_card_role(role),
                            value: WireSecret::new(value.expose().to_owned()),
                        })
                        .collect(),
                })
                .collect();
            Ok(Dispatched::Done(ResultBody::FillCard { frames: values }))
        }
        Request::SaveCard {
            url,
            top_url,
            title,
            cardholder_name,
            number,
            verification_number,
            expiry,
        } => {
            require_enabled(v)?;
            let staged = v
                .stage_save_card(
                    url,
                    top_url.as_deref(),
                    NewCard {
                        title: title.as_deref(),
                        cardholder_name: cardholder_name.as_deref(),
                        number: SecretString::new(number.expose().to_owned()),
                        verification_number: verification_number
                            .as_ref()
                            .map(|c| SecretString::new(c.expose().to_owned())),
                        expiry: expiry.as_deref(),
                    },
                    now_ms(unix_seconds),
                )
                .map_err(code)?;
            Ok(Dispatched::SaveCard(staged))
        }
```

Add `cards_for_page, card_values_for_page, stage_save_card` to the list in the module comment at the top of `dispatch.rs`.

- [ ] **Step 4: Implement `server.rs`**

In `handle`:
- Add `| Request::FindCards { .. }` to the Lookup arm.
- Add `| Request::FillCard { .. } | Request::SaveCard { .. }` to the Secret arm. Extend its comment: "`fill_card` returns a card; `save_card` writes one."
- Add the result arm after `Dispatched::SaveSso`:

```rust
            Ok(Dispatched::SaveCard(staged)) => {
                let item_id = staged.item_id;
                (self.inner.save)(staged.write).map(|()| ResultBody::SaveCard { item_id })
            }
```

- Extend the change hook: `Request::SaveLogin { .. } | Request::PasskeyCreate { .. } | Request::SaveSso { .. } | Request::SaveCard { .. }`.

In `ratelimit.rs`, extend the `RequestClass` docs: add `find_cards` to Lookup, and `fill_card`, `save_card` to Secret.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p havenkeys-bridge`
Expected: all pass.

- [ ] **Step 6: Lint and commit**

```bash
cargo clippy -p havenkeys-bridge --all-targets -- -D warnings && cargo fmt --all --check
git add crates/havenkeys-bridge
git commit -m "feat(bridge): answer card requests; processor frames, secret rate class, security tests"
```

---

### Task 4: Protocol (TypeScript) — card roles, brands, IIN table, requests, results

**Files:**
- Create: `packages/protocol/src/card.ts`
- Create: `packages/protocol/src/card-parity.test.ts`
- Modify: `packages/protocol/src/index.ts`
- Modify: `packages/protocol/src/index.test.ts`
- Modify: `packages/protocol/tsconfig.json` (add `"src/card-parity.test.ts"` to `exclude`)

**Interfaces:**
- Produces, from `@havenkeys/protocol`:
  - `CARD_ROLES`, `type CardRole`, `CARD_BRANDS`, `type CardBrandId`, `CARD_BRAND_NAMES`, `IIN_RANGES`.
  - `brandOf(digits: string): CardBrandId | null`, `luhnOk(digits: string): boolean`, `isCardRole`, `isCardBrand`.
  - `MAX_CARD_FRAMES`, `MAX_CARD_ROLES`, `MAX_CARD_VALUE_BYTES`.
  - `interface CardMatch`, `interface CardValue`, `interface CardFrameRequest`.
  - The request and result shapes for `find_cards`, `fill_card` and `save_card`.

- [ ] **Step 1: Write the failing tests**

`packages/protocol/src/card-parity.test.ts`:

```ts
// Runs under Node in vitest; excluded from tsconfig because it uses Node APIs.
// The extension's card roles and brands must be the Rust protocol's
// (message.rs), and its IIN table the core's (card.rs), in order.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { CARD_BRANDS, CARD_ROLES, IIN_RANGES } from "./card";

const MESSAGE = join(__dirname, "../../../crates/havenkeys-protocol/src/message.rs");
const CARD = join(__dirname, "../../../crates/havenkeys-core/src/card.rs");

function variants(src: string, head: string): string[] {
  const body = src.slice(src.indexOf(head));
  const block = body.slice(0, body.indexOf("}"));
  return [...block.matchAll(/^\s+([A-Z]\w*),$/gm)].map((m) => m[1] as string);
}

describe("card protocol parity", () => {
  const src = readFileSync(MESSAGE, "utf8");
  it("roles match havenkeys-protocol CardRole, in order", () => {
    expect(variants(src, "pub enum CardRole {").map((v) => v[0]!.toLowerCase() + v.slice(1))).toEqual([...CARD_ROLES]);
  });
  it("brands match havenkeys-protocol CardBrandId, in order", () => {
    expect(variants(src, "pub enum CardBrandId {").map((v) => v.toLowerCase())).toEqual([...CARD_BRANDS]);
  });
  it("the IIN table is havenkeys-core's, row for row", () => {
    const rust = readFileSync(CARD, "utf8");
    const rows = [...rust.matchAll(/IinRange \{ brand: CardBrand::(\w+), lo: "(\d+)", hi: "(\d+)" \}/g)].map((m) => ({
      brand: (m[1] as string).toLowerCase(),
      lo: m[2],
      hi: m[3],
    }));
    expect(rows.length).toBeGreaterThan(10);
    expect(IIN_RANGES.map((r) => ({ ...r }))).toEqual(rows);
  });
});
```

Append to `packages/protocol/src/index.test.ts`:

```ts
import { brandOf, luhnOk } from "./card";

describe("card helpers", () => {
  it("brandOf follows the table order: Elo and Hipercard before the wide ranges", () => {
    expect(brandOf("4111111111111111")).toBe("visa");
    expect(brandOf("5555555555554444")).toBe("mastercard");
    expect(brandOf("2221000000000009")).toBe("mastercard");
    expect(brandOf("378282246310005")).toBe("amex");
    expect(brandOf("6062825624254001")).toBe("hipercard");
    expect(brandOf("4011780000000000")).toBe("elo");
    expect(brandOf("6362970000457013")).toBe("elo");
    expect(brandOf("9999999999999999")).toBeNull();
    expect(brandOf("41x1")).toBeNull();
  });
  it("luhnOk accepts valid numbers only", () => {
    expect(luhnOk("4111111111111111")).toBe(true);
    expect(luhnOk("4111111111111112")).toBe(false);
    expect(luhnOk("4111")).toBe(false);
  });
});

describe("card results", () => {
  const id = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
  const result = (r: unknown) => parseIncoming({ v: 1, id: 1, result: r });
  it("accepts exact card results", () => {
    expect(result({ type: "find_cards", insecure: false, cards: [{ id, title: "Visa", brand: "visa", last4: "1111", expiry: "2033-11" }] })).not.toBeNull();
    expect(result({ type: "find_cards", insecure: true, cards: [] })).not.toBeNull();
    expect(result({ type: "fill_card", frames: [{ values: [{ role: "number", value: "4111111111111111" }] }, { values: [] }] })).not.toBeNull();
    expect(result({ type: "save_card", itemId: id })).not.toBeNull();
  });
  it("rejects anything else", () => {
    for (const bad of [
      { type: "find_cards", insecure: true, cards: [{ id, title: "x", brand: null, last4: null, expiry: null }] },
      { type: "find_cards", insecure: false, cards: [{ id, title: "x", brand: "nubank", last4: null, expiry: null }] },
      { type: "find_cards", insecure: false, cards: [{ id, title: "x", brand: null, last4: "123", expiry: null }] },
      { type: "find_cards", insecure: false, cards: [{ id, title: "x", brand: null, last4: null, expiry: "11/2033" }] },
      { type: "find_cards", insecure: false, cards: [{ id, title: "x", brand: null, last4: null }] },
      { type: "fill_card", frames: [{ values: [{ role: "number", value: "" }] }] },
      { type: "fill_card", frames: [{ values: [{ role: "number", value: "1" }, { role: "number", value: "2" }] }] },
      { type: "fill_card", frames: [{ values: [{ role: "password", value: "1" }] }] },
      { type: "fill_card", frames: Array.from({ length: 9 }, () => ({ values: [] })) },
      { type: "fill_card", frames: [{ values: [], extra: 1 }] },
      { type: "save_card", itemId: "x" },
    ]) {
      expect(result(bad), JSON.stringify(bad)).toBeNull();
    }
  });
});
```

`parseIncoming` is already imported in `index.test.ts`. If it is not, add it to the existing `./index` import.

- [ ] **Step 2: Run to see it fail**

Run: `cd packages/protocol && npx vitest run src/index.test.ts src/card-parity.test.ts`
Expected: FAIL; `./card` does not exist.

- [ ] **Step 3: Implement `packages/protocol/src/card.ts`**

```ts
// Card fill roles, brands and the IIN table (spec 2026-09-29-card-autofill
// §4, §6.1). CARD_ROLES and CARD_BRANDS mirror CardRole and CardBrandId in
// crates/havenkeys-protocol/src/message.rs, in order; IIN_RANGES copies
// IIN_RANGES in crates/havenkeys-core/src/card.rs row for row
// (card-parity.test.ts checks all three). brandOf is for display only (the
// save prompt's logo); what is saved uses Rust's detection.

export const CARD_ROLES = [
  "cardholderName",
  "cardholderGivenName",
  "cardholderFamilyName",
  "number",
  "verificationNumber",
  "expiryMonth",
  "expiryYear",
  "brand",
] as const;
export type CardRole = (typeof CARD_ROLES)[number];

export const CARD_BRANDS = ["visa", "mastercard", "amex", "elo", "hipercard", "diners", "discover", "jcb", "unionpay", "maestro", "other"] as const;
export type CardBrandId = (typeof CARD_BRANDS)[number];

export const CARD_BRAND_NAMES: Record<CardBrandId, string> = {
  visa: "Visa",
  mastercard: "Mastercard",
  amex: "American Express",
  elo: "Elo",
  hipercard: "Hipercard",
  diners: "Diners Club",
  discover: "Discover",
  jcb: "JCB",
  unionpay: "UnionPay",
  maestro: "Maestro",
  other: "Card",
};

export const MAX_CARD_FRAMES = 8;
export const MAX_CARD_ROLES = 8;
export const MAX_CARD_VALUE_BYTES = 1024;

export function isCardRole(v: unknown): v is CardRole {
  return typeof v === "string" && (CARD_ROLES as readonly string[]).includes(v);
}

export function isCardBrand(v: unknown): v is CardBrandId {
  return typeof v === "string" && (CARD_BRANDS as readonly string[]).includes(v);
}

export interface CardMatch {
  id: string;
  title: string;
  brand: CardBrandId | null;
  last4: string | null;
  /** YYYY-MM. */
  expiry: string | null;
}

export interface CardValue {
  role: CardRole;
  value: string;
}

export interface CardFrameRequest {
  url: string;
  roles: CardRole[];
}

export interface IinRange {
  brand: CardBrandId;
  lo: string;
  hi: string;
}

/**
 * Copied from crates/havenkeys-core/src/card.rs IIN_RANGES, same order
 * (card-parity.test.ts checks). card.rs is the authority.
 */
export const IIN_RANGES: readonly IinRange[] = [
  { brand: "elo", lo: "401178", hi: "401179" },
  { brand: "elo", lo: "431274", hi: "431274" },
  { brand: "elo", lo: "438935", hi: "438935" },
  { brand: "elo", lo: "451416", hi: "451416" },
  { brand: "elo", lo: "457393", hi: "457393" },
  { brand: "elo", lo: "457631", hi: "457632" },
  { brand: "elo", lo: "504175", hi: "504175" },
  { brand: "elo", lo: "506699", hi: "506778" },
  { brand: "elo", lo: "509000", hi: "509999" },
  { brand: "elo", lo: "627780", hi: "627780" },
  { brand: "elo", lo: "636297", hi: "636297" },
  { brand: "elo", lo: "636368", hi: "636368" },
  { brand: "elo", lo: "650031", hi: "650033" },
  { brand: "elo", lo: "650035", hi: "650051" },
  { brand: "elo", lo: "650057", hi: "650081" },
  { brand: "elo", lo: "650405", hi: "650439" },
  { brand: "elo", lo: "650485", hi: "650538" },
  { brand: "elo", lo: "650541", hi: "650598" },
  { brand: "elo", lo: "650700", hi: "650718" },
  { brand: "elo", lo: "650720", hi: "650727" },
  { brand: "elo", lo: "650901", hi: "650978" },
  { brand: "elo", lo: "651652", hi: "651704" },
  { brand: "elo", lo: "655000", hi: "655019" },
  { brand: "elo", lo: "655021", hi: "655058" },
  { brand: "hipercard", lo: "606282", hi: "606282" },
  { brand: "hipercard", lo: "384100", hi: "384100" },
  { brand: "hipercard", lo: "384140", hi: "384140" },
  { brand: "hipercard", lo: "384160", hi: "384160" },
  { brand: "hipercard", lo: "637095", hi: "637095" },
  { brand: "hipercard", lo: "637568", hi: "637568" },
  { brand: "hipercard", lo: "637599", hi: "637599" },
  { brand: "hipercard", lo: "637609", hi: "637609" },
  { brand: "hipercard", lo: "637612", hi: "637612" },
  { brand: "visa", lo: "4", hi: "4" },
  { brand: "mastercard", lo: "51", hi: "55" },
  { brand: "mastercard", lo: "2221", hi: "2720" },
  { brand: "amex", lo: "34", hi: "34" },
  { brand: "amex", lo: "37", hi: "37" },
  { brand: "diners", lo: "300", hi: "305" },
  { brand: "diners", lo: "3095", hi: "3095" },
  { brand: "diners", lo: "36", hi: "36" },
  { brand: "diners", lo: "38", hi: "39" },
  { brand: "discover", lo: "6011", hi: "6011" },
  { brand: "discover", lo: "644", hi: "649" },
  { brand: "discover", lo: "65", hi: "65" },
  { brand: "jcb", lo: "3528", hi: "3589" },
  { brand: "unionpay", lo: "62", hi: "62" },
  { brand: "unionpay", lo: "8100", hi: "8171" },
  { brand: "maestro", lo: "50", hi: "50" },
  { brand: "maestro", lo: "56", hi: "58" },
  { brand: "maestro", lo: "6304", hi: "6304" },
  { brand: "maestro", lo: "67", hi: "67" },
];

/** The brand of a number's prefix: the first row that holds it. */
export function brandOf(digits: string): CardBrandId | null {
  if (!/^[0-9]{1,19}$/.test(digits)) return null;
  for (const r of IIN_RANGES) {
    if (digits.length < r.lo.length) continue;
    const prefix = digits.slice(0, r.lo.length);
    if (prefix >= r.lo && prefix <= r.hi) return r.brand;
  }
  return null;
}

/** The card check digit (Luhn). */
export function luhnOk(digits: string): boolean {
  if (!/^[0-9]{8,19}$/.test(digits)) return false;
  let sum = 0;
  for (let i = 0; i < digits.length; i++) {
    let d = digits.charCodeAt(digits.length - 1 - i) - 48;
    if (i % 2 === 1) {
      d *= 2;
      if (d > 9) d -= 9;
    }
    sum += d;
  }
  return sum % 10 === 0;
}
```

- [ ] **Step 4: Wire the requests and results into `index.ts`**
  - After `export * from "./identity";`, add `export * from "./card";`. Add an import line: `import { isCardBrand, isCardRole, MAX_CARD_FRAMES, MAX_CARD_ROLES, MAX_CARD_VALUE_BYTES, type CardFrameRequest, type CardMatch, type CardRole, type CardValue } from "./card";`.
  - Add these members to the `Request` union:

```ts
  | { type: "find_cards"; url: string; topUrl?: string }
  | { type: "fill_card"; itemId: string; topUrl: string; frames: CardFrameRequest[] }
  | {
      type: "save_card";
      url: string;
      topUrl?: string;
      title?: string;
      cardholderName?: string;
      number: string;
      verificationNumber?: string;
      /** YYYY-MM. */
      expiry?: string;
    }
```

  - Add these members to the `Result` union:

```ts
  | { type: "find_cards"; insecure: boolean; cards: CardMatch[] }
  | { type: "fill_card"; frames: Array<{ values: CardValue[] }> }
  | { type: "save_card"; itemId: string }
```

  - Before `function parseList`, add these parsers:

```ts
const EXPIRY = /^\d{4}-(0[1-9]|1[0-2])$/;

function parseCardMatch(v: unknown): CardMatch | null {
  if (!isObj(v) || !hasExactKeys(v, ["id", "title", "brand", "last4", "expiry"])) return null;
  const { id, title, brand, last4, expiry } = v;
  if (!isUuid(id) || !isStr(title) || utf8Length(title) > 4 * 256) return null;
  if (brand !== null && !isCardBrand(brand)) return null;
  if (last4 !== null && !(isStr(last4) && /^[0-9]{4}$/.test(last4))) return null;
  if (expiry !== null && !(isStr(expiry) && EXPIRY.test(expiry))) return null;
  return { id, title, brand, last4, expiry };
}

function parseCardFrameValues(v: unknown): { values: CardValue[] } | null {
  if (!isObj(v) || !hasExactKeys(v, ["values"]) || !Array.isArray(v.values) || v.values.length > MAX_CARD_ROLES) return null;
  const values: CardValue[] = [];
  for (const x of v.values) {
    if (!isObj(x) || !hasExactKeys(x, ["role", "value"]) || !isCardRole(x.role) || !isStr(x.value)) return null;
    if (x.value.length === 0 || utf8Length(x.value) > MAX_CARD_VALUE_BYTES) return null;
    const role: CardRole = x.role;
    if (values.some((y) => y.role === role)) return null;
    values.push({ role, value: x.value });
  }
  return { values };
}
```

  - Add these cases to `parseResult`, before `default`:

```ts
    case "find_cards": {
      if (!hasExactKeys(v, ["type", "insecure", "cards"]) || !isBool(v.insecure)) return null;
      const cards = parseList(v.cards, parseCardMatch);
      if (!cards || (v.insecure && cards.length > 0)) return null;
      return { type: "find_cards", insecure: v.insecure, cards };
    }
    case "fill_card": {
      if (!hasExactKeys(v, ["type", "frames"]) || !Array.isArray(v.frames) || v.frames.length > MAX_CARD_FRAMES) return null;
      const frames: Array<{ values: CardValue[] }> = [];
      for (const f of v.frames) {
        const p = parseCardFrameValues(f);
        if (!p) return null;
        frames.push(p);
      }
      return { type: "fill_card", frames };
    }
    case "save_card":
      if (!hasExactKeys(v, ["type", "itemId"]) || !isUuid(v.itemId)) return null;
      return { type: "save_card", itemId: v.itemId };
```

  - Add `"src/card-parity.test.ts"` to `exclude` in `packages/protocol/tsconfig.json`.

- [ ] **Step 5: Run the tests**

Run: `cd packages/protocol && npx vitest run && npx tsc --noEmit -p .`
Expected: all pass. The table above is card.rs's as the card-item plan writes it; if card.rs changed since, the parity test names the row, and card.rs's rows are copied here verbatim.

- [ ] **Step 6: Commit**

```bash
git add packages/protocol
git commit -m "feat(protocol-ts): card roles, brands, IIN table, requests and results, with parity tests"
```

---

### Task 5: Extension — card field classifier

**Files:**
- Create: `apps/extension/src/autofill/card.ts`
- Test: `apps/extension/src/autofill/card.test.ts`

**Interfaces:**
- Consumes: `groupRoot`, `MAX_GROUP_INPUTS`, `type Env` (`./group`); `hasAny`, `MAX_HINT_CHARS`, `normalize` (`./text`); `MAX_CARD_ROLES`, `type CardRole` (`@havenkeys/protocol`).
- Produces:
  - Types:
    - `type CardElement = HTMLInputElement | HTMLSelectElement`.
    - `type CardFieldKind = CardRole | "expiry"`.
    - `type CardShape = { kind: "whole" } | { kind: "slice"; start: number; length: number }`.
    - `interface CardField { el: CardElement; kind: CardFieldKind; shape: CardShape; confidence: number; byAutocomplete: boolean }`.
    - `interface CardGroup { root: ParentNode; fields: CardField[] }`.
  - Functions:
    - `cardKindOf(el: CardElement, strong: boolean): { kind: CardFieldKind; confidence: number; byAutocomplete: boolean } | null`.
    - `isCardFillable(el: CardElement, env: Env): boolean`.
    - `classifyCard(root: ParentNode, env: Env): CardField[]`.
    - `cardGroupFor(field: HTMLInputElement, env: Env): CardGroup | null`.
    - `findCardGroup(root: ParentNode, env: Env): CardGroup | null`.
    - `cardFieldsForFill(doc: Document, env: Env): CardGroup | null`.
    - `cardRolesOf(fields: readonly CardField[]): CardRole[]`.

- [ ] **Step 1: Write the failing tests** (`apps/extension/src/autofill/card.test.ts`)

```ts
// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import type { Env } from "./group";
import { groupFor } from "./group";
import { identityRoleOf } from "./identity";
import { cardFieldsForFill, cardGroupFor, cardRolesOf } from "./card";

const env: Env = { isVisible: (el) => !el.hidden, path: "/" };
const page = (html: string) => (document.body.innerHTML = html);
const $ = <T extends HTMLElement = HTMLInputElement>(sel: string) => document.querySelector(sel) as T;
const kinds = (sel: string) => cardGroupFor($(sel), env)?.fields.map((f) => [f.el.id, f.kind]) ?? null;
const months = Array.from({ length: 12 }, (_, i) => `<option value="${i + 1}">${String(i + 1).padStart(2, "0")}</option>`).join("");
const years = Array.from({ length: 12 }, (_, i) => `<option>${2026 + i}</option>`).join("");

beforeEach(() => page(""));

describe("card classifier", () => {
  it("reads a Brazilian checkout by its words", () => {
    page(`<form>
      <input id="num" name="numero_cartao" aria-label="Número do cartão" inputmode="numeric" maxlength="19">
      <input id="nome" name="nome_impresso" aria-label="Nome impresso no cartão">
      <input id="val" name="validade" placeholder="MM/AA" maxlength="5">
      <input id="cvv" name="cvv" aria-label="CVV" maxlength="4">
      <button>Pagar</button></form>`);
    expect(kinds("#num")).toEqual([["num", "number"], ["nome", "cardholderName"], ["val", "expiry"], ["cvv", "verificationNumber"]]);
    expect(cardRolesOf(cardGroupFor($("#num"), env)!.fields)).toEqual(["number", "cardholderName", "expiryMonth", "expiryYear", "verificationNumber"]);
  });

  it("reads a US checkout by autocomplete, month and year selects included", () => {
    page(`<form>
      <input id="cn" autocomplete="cc-name"><input id="cc" autocomplete="billing cc-number">
      <select id="m" autocomplete="cc-exp-month">${months}</select><select id="y" autocomplete="cc-exp-year">${years}</select>
      <input id="csc" type="password" autocomplete="cc-csc" maxlength="4"></form>`);
    expect(kinds("#cc")).toEqual([["cn", "cardholderName"], ["cc", "number"], ["m", "expiryMonth"], ["y", "expiryYear"], ["csc", "verificationNumber"]]);
  });

  it("finds unnamed month and year selects only beside a card number", () => {
    page(`<form><input id="num" aria-label="Card number"><select id="m" name="mm">${months}</select><select id="y" name="yy">${years}</select></form>`);
    expect(kinds("#num")).toEqual([["num", "number"], ["m", "expiryMonth"], ["y", "expiryYear"]]);
    // A birth month on a sign-up form is no card field.
    page(`<form><input id="n" name="nome" aria-label="Nome"><select id="m" name="mes">${months}</select></form>`);
    expect(kinds("#n")).toBeNull();
  });

  it("a processor frame's lone field qualifies when autocomplete names it", () => {
    page(`<input id="n" name="cardnumber" autocomplete="cc-number" inputmode="numeric">`);
    expect(kinds("#n")).toEqual([["n", "number"]]);
    page(`<input id="c" name="cvc" autocomplete="cc-csc">`);
    expect(kinds("#c")).toEqual([["c", "verificationNumber"]]);
    // A lone "month" box with no card context is nothing.
    page(`<input id="m" name="month">`);
    expect(kinds("#m")).toBeNull();
  });

  it("splits a number typed into four boxes, and Amex's 4-6-5", () => {
    page(`<form><label for="c1">Card number</label><input id="c1" maxlength="4"><input id="c2" maxlength="4"><input id="c3" maxlength="4"><input id="c4" maxlength="4"><input id="cvv" name="cvv" maxlength="3"></form>`);
    const g = cardGroupFor($("#c1"), env)!;
    expect(g.fields.filter((f) => f.kind === "number").map((f) => f.shape)).toEqual([
      { kind: "slice", start: 0, length: 4 },
      { kind: "slice", start: 4, length: 4 },
      { kind: "slice", start: 8, length: 4 },
      { kind: "slice", start: 12, length: 4 },
    ]);
    page(`<form><label for="a1">Card number</label><input id="a1" maxlength="4"><input id="a2" maxlength="6"><input id="a3" maxlength="5"></form>`);
    expect(cardGroupFor($("#a1"), env)!.fields.map((f) => f.shape)).toEqual([
      { kind: "slice", start: 0, length: 4 },
      { kind: "slice", start: 4, length: 6 },
      { kind: "slice", start: 10, length: 5 },
    ]);
  });

  it("refuses gift cards, search boxes, hidden and read-only fields", () => {
    page(`<form><input id="g" aria-label="Gift card number"><input id="s" type="search" aria-label="Card number">
      <input id="h" autocomplete="cc-number" hidden><input id="r" autocomplete="cc-number" readonly></form>`);
    for (const id of ["#g", "#s", "#h", "#r"]) expect(kinds(id), id).toBeNull();
  });

  it("the login and identity classifiers never claim a card field", () => {
    page(`<form><input id="num" name="numero_cartao" aria-label="Número do cartão"><input id="nome" aria-label="Nome impresso no cartão"><input id="cvv" name="cvv"></form>`);
    expect(groupFor($("#num"), env).kind).toBe("unknown");
    expect(identityRoleOf($("#nome"))).toBeNull();
    expect(identityRoleOf($("#cvv"))).toBeNull();
  });

  it("for a frame the user did not click, fills every card field of the page", () => {
    page(`<input id="e" autocomplete="cc-exp">`);
    expect(cardFieldsForFill(document, env)?.fields.map((f) => f.kind)).toEqual(["expiry"]);
    page(`<input id="x" name="q">`);
    expect(cardFieldsForFill(document, env)).toBeNull();
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/autofill/card.test.ts`
Expected: FAIL; `./card` does not exist.

- [ ] **Step 3: Implement `apps/extension/src/autofill/card.ts`**

```ts
// Card field classification (spec 2026-09-29-card-autofill §5.1).
//
// Separate from the login and identity classifiers, which keep refusing
// card fields (isCardField, identity NEGATIVE): a card field only ever gets
// the card menu. Pure reads of attributes compared against fixed keyword
// lists; nothing from the page is evaluated or inserted. Bounded like login
// groups: at most MAX_GROUP_INPUTS elements per group.

import { MAX_CARD_ROLES, type CardRole } from "@havenkeys/protocol";
import { groupRoot, MAX_GROUP_INPUTS, type Env } from "./group";
import { hasAny, MAX_HINT_CHARS, normalize } from "./text";

export type CardElement = HTMLInputElement | HTMLSelectElement;
/** A role, or month and year together in one field. */
export type CardFieldKind = CardRole | "expiry";
export type CardShape = { kind: "whole" } | { kind: "slice"; start: number; length: number };

export interface CardField {
  el: CardElement;
  kind: CardFieldKind;
  shape: CardShape;
  confidence: number;
  byAutocomplete: boolean;
}

export interface CardGroup {
  root: ParentNode;
  fields: CardField[];
}

/** Input types that can hold a card value. `password`: some sites hide the CVV. */
const TEXT_TYPES = new Set(["text", "tel", "number", "password", ""]);

const AUTOCOMPLETE: Record<string, CardFieldKind> = {
  "cc-name": "cardholderName",
  "cc-given-name": "cardholderGivenName",
  "cc-family-name": "cardholderFamilyName",
  "cc-number": "number",
  "cc-csc": "verificationNumber",
  "cc-exp": "expiry",
  "cc-exp-month": "expiryMonth",
  "cc-exp-year": "expiryYear",
  "cc-type": "brand",
};
const AC_PREFIX = /^(section-\S+|shipping|billing)$/;

/** Words per kind, English and Portuguese, normalized. Order matters: first hit wins. */
const WORDS: Array<[CardFieldKind, readonly string[]]> = [
  ["verificationNumber", ["cvv", "cvc", "csc", "cvv2", "cvc2", "cid", "security code", "codigo de seguranca", "cod seguranca", "card code", "card verification"]],
  ["cardholderName", ["name on card", "nome impresso", "nome impresso no cartao", "nome no cartao", "nome do titular", "titular", "cardholder", "card holder", "holder name"]],
  ["number", ["card number", "numero do cartao", "numero cartao", "cardnumber", "ccnumber", "cc number", "credit card", "cartao de credito", "card no"]],
  ["expiryMonth", ["exp month", "expiry month", "expiration month", "mes de validade", "mes validade", "mes de vencimento"]],
  ["expiryYear", ["exp year", "expiry year", "expiration year", "ano de validade", "ano validade", "ano de vencimento"]],
  ["expiry", ["expiry", "expiration", "exp date", "expiry date", "expiration date", "valid thru", "validade", "vencimento", "data de validade", "mm aa", "mm yy"]],
  ["brand", ["card type", "bandeira", "card brand"]],
];

/** Weak words: only in a group that already has a number, CVV or expiry field. */
const MONTH_WORDS = ["month", "mes", "mm"];
const YEAR_WORDS = ["year", "ano", "yy", "yyyy", "aa", "aaaa"];
const CODE_WORDS = ["code", "codigo", "cod"];

/** Wording that means "not a payment card". */
const NEGATIVE = ["gift card", "cartao presente", "vale presente", "loyalty", "fidelidade", "coupon", "cupom", "promo", "search", "busca", "pesquisar"];

const MONTH_NAMES = ["jan", "feb", "fev", "mar", "apr", "abr", "may", "mai", "jun", "jul", "aug", "ago", "sep", "set", "oct", "out", "nov", "dec", "dez"];

function attr(el: Element, n: string): string {
  return (el.getAttribute(n) ?? "").slice(0, MAX_HINT_CHARS);
}

function labelText(el: CardElement): string {
  const parts: string[] = [];
  for (const l of Array.from(el.labels ?? []).slice(0, 2)) parts.push((l.textContent ?? "").slice(0, MAX_HINT_CHARS));
  return parts.join(" ");
}

function autocompleteKind(el: CardElement): CardFieldKind | null {
  const tokens = attr(el, "autocomplete").toLowerCase().split(/\s+/).filter(Boolean).filter((t) => !AC_PREFIX.test(t));
  const last = tokens[tokens.length - 1];
  return last ? (AUTOCOMPLETE[last] ?? null) : null;
}

function optionTexts(s: HTMLSelectElement): string[] {
  return Array.from(s.options)
    .slice(0, 120)
    .map((o) => normalize(`${o.value} ${o.textContent ?? ""}`));
}

/** Options that name a month: 1–12 (padded or not) or a month name. */
function monthOptions(s: HTMLSelectElement): number {
  return optionTexts(s).filter((t) => t.split(" ").some((w) => /^(0?[1-9]|1[0-2])$/.test(w) || MONTH_NAMES.some((m) => w.startsWith(m)))).length;
}

/** Options that name a year: 2 digits, or 20xx. */
function yearOptions(s: HTMLSelectElement): number {
  return optionTexts(s).filter((t) => t.split(" ").some((w) => /^(20)?\d{2}$/.test(w))).length;
}

/** The field's card kind, or null. `strong`: the group has a number, CVV or expiry field. */
export function cardKindOf(el: CardElement, strong: boolean): { kind: CardFieldKind; confidence: number; byAutocomplete: boolean } | null {
  const select = el instanceof HTMLSelectElement;
  const type = select ? "select" : (el as HTMLInputElement).type.toLowerCase();
  if (!select && !TEXT_TYPES.has(type)) return null;
  if (attr(el, "role") === "search" || el.closest('[role="search"]')) return null;
  const attrs = normalize(`${attr(el, "name")} ${attr(el, "id")}`);
  const text = normalize(`${attr(el, "placeholder")} ${attr(el, "aria-label")} ${attr(el, "title")} ${labelText(el)}`, MAX_HINT_CHARS * 3);
  const all = `${attrs} ${text}`;
  if (hasAny(all, NEGATIVE)) return null;

  // A password box only ever holds the CVV; a select holds month, year or brand.
  const allowed = (k: CardFieldKind): boolean =>
    (type !== "password" || k === "verificationNumber") && (!select || k === "expiryMonth" || k === "expiryYear" || k === "brand");

  const fromAc = autocompleteKind(el);
  if (fromAc) return allowed(fromAc) ? { kind: fromAc, confidence: 1, byAutocomplete: true } : null;
  for (const [kind, words] of WORDS) {
    const confidence = hasAny(attrs, words) ? 0.7 : hasAny(text, words) ? 0.6 : 0;
    if (confidence === 0) continue;
    return allowed(kind) ? { kind, confidence, byAutocomplete: false } : null;
  }
  if (!strong) return null;
  if (select) {
    const s = el as HTMLSelectElement;
    if (monthOptions(s) >= 12) return { kind: "expiryMonth", confidence: 0.5, byAutocomplete: false };
    if (yearOptions(s) >= 10) return { kind: "expiryYear", confidence: 0.5, byAutocomplete: false };
  }
  if (hasAny(all, MONTH_WORDS) && allowed("expiryMonth")) return { kind: "expiryMonth", confidence: 0.5, byAutocomplete: false };
  if (hasAny(all, YEAR_WORDS) && allowed("expiryYear")) return { kind: "expiryYear", confidence: 0.5, byAutocomplete: false };
  const max = select ? -1 : (el as HTMLInputElement).maxLength;
  if (max >= 3 && max <= 4 && hasAny(all, CODE_WORDS)) return { kind: "verificationNumber", confidence: 0.5, byAutocomplete: false };
  return null;
}

/** Visible, enabled, editable. The identity's stricter visibility applies: cards are not site-bound either. */
export function isCardFillable(el: CardElement, env: Env): boolean {
  if (el.disabled) return false;
  if (!(env.identityVisible ? env.identityVisible(el) : env.isVisible(el))) return false;
  if (el instanceof HTMLSelectElement) return true;
  return !el.readOnly && TEXT_TYPES.has(el.type.toLowerCase());
}

/** A number typed into 3–5 adjacent boxes of 4–6 characters: one slice each. */
function splitNumbers(fields: CardField[], els: CardElement[]): CardField[] {
  const first = fields.find((f) => f.kind === "number");
  if (!first || !(first.el instanceof HTMLInputElement)) return fields;
  const run: HTMLInputElement[] = [];
  for (let i = els.indexOf(first.el); i >= 0 && i < els.length && run.length < 5; i++) {
    const el = els[i];
    if (!(el instanceof HTMLInputElement) || el.maxLength < 4 || el.maxLength > 6) break;
    const known = fields.find((f) => f.el === el);
    if (known && known.kind !== "number") break;
    run.push(el);
  }
  const total = run.reduce((n, el) => n + el.maxLength, 0);
  if (run.length < 3 || total < 13 || total > 19) return fields;
  const slices = new Map<CardElement, CardField>();
  let start = 0;
  for (const el of run) {
    slices.set(el, { el, kind: "number", shape: { kind: "slice", start, length: el.maxLength }, confidence: first.confidence, byAutocomplete: first.byAutocomplete });
    start += el.maxLength;
  }
  const out: CardField[] = [];
  for (const el of els) {
    const f = slices.get(el) ?? fields.find((x) => x.el === el);
    if (f) out.push(f);
  }
  return out;
}

/** Every card field under `root`, in document order, bounded. */
export function classifyCard(root: ParentNode, env: Env): CardField[] {
  const els = Array.from(root.querySelectorAll<CardElement>("input, select"))
    .slice(0, MAX_GROUP_INPUTS * 4)
    .filter((el) => isCardFillable(el, env))
    .slice(0, MAX_GROUP_INPUTS);
  const first = els.map((el) => ({ el, r: cardKindOf(el, false) }));
  const strong = first.some((x) => x.r !== null && (x.r.kind === "number" || x.r.kind === "verificationNumber" || x.r.kind === "expiry"));
  const fields: CardField[] = [];
  for (const { el, r } of first) {
    const k = r ?? (strong ? cardKindOf(el, true) : null);
    if (k) fields.push({ el, kind: k.kind, shape: { kind: "whole" }, confidence: k.confidence, byAutocomplete: k.byAutocomplete });
  }
  return splitNumbers(fields, els);
}

/** A number field; or a CVV and an expiry; or (a processor's frame) fields named by cc-* autocomplete alone. */
function qualifies(fields: readonly CardField[]): boolean {
  const kinds = new Set(fields.map((f) => f.kind));
  if (kinds.has("number")) return true;
  if (kinds.has("verificationNumber") && (kinds.has("expiry") || kinds.has("expiryMonth"))) return true;
  return fields.length > 0 && fields.every((f) => f.byAutocomplete);
}

/** The card group around a field the user interacted with, if it qualifies. */
export function cardGroupFor(field: HTMLInputElement, env: Env): CardGroup | null {
  const root = groupRoot(field);
  const fields = classifyCard(root, env);
  if (!fields.some((f) => f.el === field) || !qualifies(fields)) return null;
  return { root, fields };
}

/** The first qualifying card group under `root` (a submit's form, the popup's page), bounded. */
export function findCardGroup(root: ParentNode, env: Env): CardGroup | null {
  const seen = new Set<ParentNode>();
  for (const input of Array.from(root.querySelectorAll<HTMLInputElement>("input")).slice(0, MAX_GROUP_INPUTS)) {
    const r = groupRoot(input);
    if (seen.has(r)) continue;
    seen.add(r);
    const fields = classifyCard(r, env);
    if (qualifies(fields)) return { root: r, fields };
  }
  return null;
}

/**
 * What a frame fills when the user picked a card elsewhere in the tab (or
 * from the popup): its first card group, else every card field of the page
 * (a processor frame holding only the expiry box).
 */
export function cardFieldsForFill(doc: Document, env: Env): CardGroup | null {
  const group = findCardGroup(doc, env);
  if (group) return group;
  const fields = classifyCard(doc, env);
  return fields.length > 0 ? { root: doc, fields } : null;
}

/** The roles the fields ask Rust for, unique, in document order. */
export function cardRolesOf(fields: readonly CardField[]): CardRole[] {
  const out: CardRole[] = [];
  const add = (r: CardRole) => {
    if (!out.includes(r)) out.push(r);
  };
  for (const f of fields) {
    if (f.kind === "expiry") {
      add("expiryMonth");
      add("expiryYear");
    } else add(f.kind);
  }
  return out.slice(0, MAX_CARD_ROLES);
}
```

- [ ] **Step 4: Run the tests**

Run: `cd apps/extension && npx vitest run src/autofill/card.test.ts src/autofill/autofill.test.ts src/autofill/identity.test.ts`
Expected: all pass. The existing classifiers are unchanged.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/autofill/card.ts apps/extension/src/autofill/card.test.ts
git commit -m "feat(extension): card field classifier: words, autocomplete, selects, split numbers"
```

---

### Task 6: Extension — writing card values and reading a typed card

**Files:**
- Create: `apps/extension/src/autofill/card-fill.ts`
- Modify: `apps/extension/src/autofill/fill.ts` (add `typedByUser`)
- Test: `apps/extension/src/autofill/card-fill.test.ts`

**Interfaces:**
- Consumes: Task 5. From `./fill`: `markUserEdit` and the new `typedByUser`. From `@havenkeys/protocol`: `luhnOk`, `type CardValue`.
- Produces:
  - `expiryFor(el: HTMLInputElement, month: string, year: string): string`.
  - `matchCardOption(select: HTMLSelectElement, kind: "expiryMonth" | "expiryYear" | "brand", value: string): number`.
  - `fillCard(group: CardGroup, values: readonly CardValue[], env: Env): number`.
  - `cardRolesToFill(group: CardGroup, env: Env): CardRole[]`.
  - `interface SubmittedCard { number: string; expiry: string; verificationNumber: string | null; cardholderName: string | null }`.
  - `readCardSubmission(group: CardGroup): SubmittedCard | null`.
  - In `fill.ts`: `typedByUser(el: HTMLInputElement, key: (v: string) => string): boolean`.

- [ ] **Step 1: Write the failing tests** (`apps/extension/src/autofill/card-fill.test.ts`)

```ts
// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import type { CardValue } from "@havenkeys/protocol";
import type { Env } from "./group";
import { cardGroupFor } from "./card";
import { cardRolesToFill, expiryFor, fillCard, matchCardOption, readCardSubmission } from "./card-fill";
import { markUserEdit } from "./fill";

let hidden = new Set<string>();
const env: Env = { isVisible: (el) => !el.hidden && !hidden.has(el.id), path: "/" };
const page = (html: string) => (document.body.innerHTML = html);
const $ = <T extends HTMLElement = HTMLInputElement>(sel: string) => document.querySelector(sel) as T;
const input = (attrs: string) => {
  page(`<input id="x" ${attrs}>`);
  return $("#x");
};

const VALUES: CardValue[] = [
  { role: "cardholderName", value: "Samuel S Rocha" },
  { role: "cardholderGivenName", value: "Samuel" },
  { role: "cardholderFamilyName", value: "S Rocha" },
  { role: "number", value: "4111111111111111" },
  { role: "verificationNumber", value: "123" },
  { role: "expiryMonth", value: "4" },
  { role: "expiryYear", value: "2033" },
  { role: "brand", value: "visa" },
];

beforeEach(() => {
  page("");
  hidden = new Set();
});

describe("expiry in one field", () => {
  it("follows the placeholder, then maxlength, then MM/YY", () => {
    expect(expiryFor(input(`placeholder="MM/AA"`), "4", "2033")).toBe("04/33");
    expect(expiryFor(input(`placeholder="MM / YY"`), "4", "2033")).toBe("04 / 33");
    expect(expiryFor(input(`placeholder="MM/YYYY"`), "11", "2033")).toBe("11/2033");
    expect(expiryFor(input(`placeholder="mm-yy"`), "11", "2033")).toBe("11-33");
    expect(expiryFor(input(`placeholder="MMYY"`), "11", "2033")).toBe("1133");
    expect(expiryFor(input(`maxlength="4"`), "4", "2033")).toBe("0433");
    expect(expiryFor(input(`maxlength="7"`), "4", "2033")).toBe("04/2033");
    expect(expiryFor(input(``), "4", "2033")).toBe("04/33");
  });
});

describe("selects", () => {
  it("month selects by number, name and mixed text; never the placeholder", () => {
    page(`<select id="a"><option value="">Mês</option><option value="1">01</option><option value="4">04</option></select>
      <select id="b"><option>Month</option><option value="03">03 - Março</option><option value="04">04 - Abril</option></select>
      <select id="c"><option value="">--</option><option value="jan">January</option><option value="apr">April</option></select>`);
    expect(matchCardOption($("#a"), "expiryMonth", "4")).toBe(2);
    expect(matchCardOption($("#b"), "expiryMonth", "4")).toBe(2);
    expect(matchCardOption($("#c"), "expiryMonth", "4")).toBe(2);
    expect(matchCardOption($("#a"), "expiryMonth", "12")).toBe(-1);
  });
  it("year selects by 2 or 4 digits; brand selects by id, name or code", () => {
    page(`<select id="y"><option value="">Ano</option><option value="32">32</option><option value="33">33</option></select>
      <select id="b"><option value="">--</option><option value="MC">Master</option><option value="VI">Visa</option></select>`);
    expect(matchCardOption($("#y"), "expiryYear", "2033")).toBe(2);
    expect(matchCardOption($("#b"), "brand", "visa")).toBe(2);
    expect(matchCardOption($("#b"), "brand", "mastercard")).toBe(1);
    expect(matchCardOption($("#b"), "brand", "elo")).toBe(-1);
  });
});

describe("fillCard", () => {
  const FORM = `<form>
    <input id="num" name="numero_cartao" aria-label="Número do cartão" placeholder="0000 0000 0000 0000" maxlength="19">
    <input id="nome" name="nome_impresso" aria-label="Nome impresso no cartão">
    <input id="val" name="validade" placeholder="MM/AA" maxlength="5">
    <input id="cvv" type="password" name="cvv" maxlength="3">
  </form>`;
  const group = () => cardGroupFor($("#num"), env)!;

  it("fills each field in its shape and fires input and change", () => {
    page(FORM);
    const events: string[] = [];
    $("#val").addEventListener("input", () => events.push("input"));
    $("#val").addEventListener("change", () => events.push("change"));
    expect(fillCard(group(), VALUES, env)).toBe(4);
    expect($("#num").value).toBe("4111 1111 1111 1111");
    expect($("#nome").value).toBe("Samuel S Rocha");
    expect($("#val").value).toBe("04/33");
    expect($("#cvv").value).toBe("123");
    expect(events).toEqual(["input", "change"]);
    expect(document.body.innerHTML).not.toContain("4111");
  });

  it("leaves an Amex code out of a 3-character CVV field instead of cutting it", () => {
    page(FORM);
    fillCard(group(), VALUES.map((v) => (v.role === "verificationNumber" ? { ...v, value: "1234" } : v)), env);
    expect($("#cvv").value).toBe("");
  });

  it("fills month and year selects, padded text months and names split in two", () => {
    page(`<form><input id="num" autocomplete="cc-number"><input id="g" autocomplete="cc-given-name"><input id="f" autocomplete="cc-family-name">
      <input id="m" autocomplete="cc-exp-month" maxlength="2"><select id="y" autocomplete="cc-exp-year"><option value="">Ano</option><option>2033</option></select></form>`);
    fillCard(cardGroupFor($("#num"), env)!, VALUES, env);
    expect($("#g").value).toBe("Samuel");
    expect($("#f").value).toBe("S Rocha");
    expect($("#m").value).toBe("04");
    expect($<HTMLSelectElement>("#y").value).toBe("2033");
  });

  it("fills a split number slice by slice", () => {
    page(`<form><label for="c1">Card number</label><input id="c1" maxlength="4"><input id="c2" maxlength="4"><input id="c3" maxlength="4"><input id="c4" maxlength="4"></form>`);
    fillCard(cardGroupFor($("#c1"), env)!, VALUES, env);
    expect(["#c1", "#c2", "#c3", "#c4"].map((s) => $(s).value)).toEqual(["4111", "1111", "1111", "1111"]);
  });

  it("never overwrites what the user typed, but updates its own values", () => {
    page(FORM);
    $("#nome").value = "Typed";
    fillCard(group(), VALUES, env);
    expect($("#nome").value).toBe("Typed");
    expect(fillCard(group(), VALUES.map((v) => (v.role === "number" ? { ...v, value: "5555555555554444" } : v)), env)).toBeGreaterThan(0);
    expect($("#num").value).toBe("5555 5555 5555 4444");
  });

  it("skips a field hidden after the menu opened", () => {
    page(FORM);
    const g = group();
    hidden.add("cvv");
    fillCard(g, VALUES, env);
    expect($("#cvv").value).toBe("");
  });

  it("goes around an instance value setter (React's tracker) and still announces", () => {
    page(FORM);
    const el = $("#nome");
    let instanceSets = 0;
    Object.defineProperty(el, "value", {
      configurable: true,
      get: () => Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.get!.call(el),
      set: () => void instanceSets++,
    });
    fillCard(group(), VALUES, env);
    expect(instanceSets).toBe(0);
    expect(el.value).toBe("Samuel S Rocha");
  });

  it("asks only for the roles of fields it could write now", () => {
    page(FORM);
    $("#cvv").value = "999";
    expect(cardRolesToFill(group(), env)).toEqual(["number", "cardholderName", "expiryMonth", "expiryYear"]);
  });
});

describe("readCardSubmission", () => {
  const FORM = `<form><input id="num" autocomplete="cc-number"><input id="exp" autocomplete="cc-exp"><input id="cvv" autocomplete="cc-csc"><input id="n" autocomplete="cc-name"></form>`;
  const typed = (sel: string, v: string) => {
    $(sel).value = v;
    markUserEdit($(sel));
  };

  it("reads a card the user typed", () => {
    page(FORM);
    typed("#num", "4111 1111 1111 1111");
    typed("#exp", "04/33");
    typed("#cvv", "123");
    typed("#n", " Samuel Rocha ");
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)).toEqual({
      number: "4111111111111111",
      expiry: "2033-04",
      verificationNumber: "123",
      cardholderName: "Samuel Rocha",
    });
  });

  it("offers to save a masked number the user typed", () => {
    page(FORM);
    typed("#num", "41111111111111111".slice(0, 16));
    // The site's mask reformats after the user's input event.
    $("#num").value = "4111 1111 1111 1111";
    typed("#exp", "0433");
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)?.number).toBe("4111111111111111");
  });

  it("ignores numbers a page script wrote, failing numbers and missing expiries", () => {
    page(FORM);
    $("#num").value = "4111111111111111"; // no user edit
    typed("#exp", "04/33");
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)).toBeNull();
    typed("#num", "4111111111111112");
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)).toBeNull();
    typed("#num", "4111111111111111");
    typed("#exp", "");
    expect(readCardSubmission(cardGroupFor($("#num"), env)!)).toBeNull();
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/autofill/card-fill.test.ts`
Expected: FAIL; `./card-fill` does not exist.

- [ ] **Step 3: Add `typedByUser` to `fill.ts`** (after `markUserEdit`)

```ts
/**
 * The user typed the field's current value, compared through `key`: a site
 * that reformats as the user types (a card mask adding spaces) keeps the
 * same digits. Page-script values never count.
 */
export function typedByUser(el: HTMLInputElement, key: (v: string) => string): boolean {
  const s = sources.get(el);
  return s !== undefined && s.source === "user" && key(s.value) === key(el.value);
}
```

- [ ] **Step 4: Implement `apps/extension/src/autofill/card-fill.ts`**

```ts
// Writing card values into a checkout (spec 2026-09-29-card-autofill §5.4),
// and reading a card the user typed (§5.6).
//
// Only the group the user picked from (or, for other frames of the tab,
// their card fields); only empty fields or fields whose value we wrote; each
// field re-checked just before writing. Values go into `value` /
// `selectedIndex` only, never attributes, and nothing is logged. Rust sends
// whole values; the shapes (MM/YY, slices, option text) are made here.

import { luhnOk, type CardRole, type CardValue } from "@havenkeys/protocol";
import type { Env } from "./group";
import { cardRolesOf, isCardFillable, type CardElement, type CardField, type CardGroup } from "./card";
import { typedByUser } from "./fill";
import { normalize } from "./text";

/** What we last wrote into each element: a value equal to it is ours to replace. */
const written = new WeakMap<CardElement, string>();
const inputSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;

/** Month names and abbreviations, English and Portuguese, normalized; index 0 = January. */
const MONTHS: ReadonlyArray<readonly string[]> = [
  ["january", "jan", "janeiro"],
  ["february", "feb", "fevereiro", "fev"],
  ["march", "mar", "marco"],
  ["april", "apr", "abril", "abr"],
  ["may", "maio", "mai"],
  ["june", "jun", "junho"],
  ["july", "jul", "julho"],
  ["august", "aug", "agosto", "ago"],
  ["september", "sep", "sept", "setembro", "set"],
  ["october", "oct", "outubro", "out"],
  ["november", "nov", "novembro"],
  ["december", "dec", "dezembro", "dez"],
];

/** Every way a brand select may name a brand, normalized. */
const BRAND_ALIASES: Record<string, readonly string[]> = {
  visa: ["visa", "vi"],
  mastercard: ["mastercard", "master card", "master", "mc"],
  amex: ["amex", "american express", "ax"],
  elo: ["elo"],
  hipercard: ["hipercard", "hiper", "hc"],
  diners: ["diners", "diners club", "dc"],
  discover: ["discover", "di"],
  jcb: ["jcb"],
  unionpay: ["unionpay", "union pay", "cup"],
  maestro: ["maestro"],
  other: [],
};

const hint = (el: Element) => `${el.getAttribute("placeholder") ?? ""}`.slice(0, 200).toLowerCase();

/** Month and year in one field, in the format its placeholder shows; else by maxlength; else MM/YY. */
export function expiryFor(el: HTMLInputElement, month: string, year: string): string {
  const mm = month.padStart(2, "0");
  const yy = year.slice(-2);
  const m = /mm(\s*[/.-]\s*|)(yyyy|aaaa|yy|aa)(?![a-z])/.exec(hint(el));
  if (m) return `${mm}${m[1] ?? ""}${(m[2] ?? "").length === 4 ? year : yy}`;
  switch (el.maxLength) {
    case 4:
      return `${mm}${yy}`;
    case 7:
      return `${mm}/${year}`;
    default:
      return `${mm}/${yy}`;
  }
}

function monthText(el: HTMLInputElement, month: string): string {
  return el.maxLength === 2 || /\bmm\b/.test(hint(el)) ? month.padStart(2, "0") : month;
}

function yearText(el: HTMLInputElement, year: string): string {
  return el.maxLength === 2 || /^\s*(yy|aa)\s*$/.test(hint(el)) ? year.slice(-2) : year;
}

/** Digits, grouped with spaces when the placeholder shows groups (Amex 4-6-5). */
function numberText(el: HTMLInputElement, digits: string): string {
  const grouped = /\d{4}\s\d|[x•*]{4}\s[x•*]/i.test(hint(el));
  if (!grouped) return digits;
  const amex = digits.length === 15 && /^3[47]/.test(digits);
  const parts = amex ? [digits.slice(0, 4), digits.slice(4, 10), digits.slice(10)] : (digits.match(/.{1,4}/g) ?? [digits]);
  const spaced = parts.join(" ");
  return el.maxLength < 0 || spaced.length <= el.maxLength ? spaced : digits;
}

/** The index of the option matching `value` by value or text, or −1. Never a disabled or placeholder option. */
export function matchCardOption(select: HTMLSelectElement, kind: "expiryMonth" | "expiryYear" | "brand", value: string): number {
  const wanted = new Set<string>();
  if (kind === "expiryMonth") {
    const n = Number(value);
    wanted.add(String(n));
    wanted.add(String(n).padStart(2, "0"));
    for (const name of MONTHS[n - 1] ?? []) wanted.add(name);
  } else if (kind === "expiryYear") {
    wanted.add(value);
    wanted.add(value.slice(-2));
  } else {
    for (const a of BRAND_ALIASES[value] ?? []) wanted.add(normalize(a));
  }
  const matches = (raw: string): boolean => {
    const t = normalize(raw);
    if (t === "") return false;
    if (wanted.has(t)) return true;
    // "03 - Março": every word names the same month.
    return kind === "expiryMonth" && t.split(" ").every((w) => wanted.has(w));
  };
  return Array.from(select.options)
    .slice(0, 500)
    .findIndex(
      (o) => !o.disabled && !(o.parentElement instanceof HTMLOptGroupElement && o.parentElement.disabled) && (matches(o.value) || matches(o.textContent ?? "")),
    );
}

/** What a field gets from the values, in its shape, or undefined. */
function valueFor(f: CardField, byRole: ReadonlyMap<CardRole, string>): string | undefined {
  const el = f.el;
  const month = byRole.get("expiryMonth");
  const year = byRole.get("expiryYear");
  const text = el instanceof HTMLInputElement ? el : null;
  switch (f.kind) {
    case "expiry":
      return text && month && year ? expiryFor(text, month, year) : undefined;
    case "expiryMonth":
      return month && text ? monthText(text, month) : month;
    case "expiryYear":
      return year && text ? yearText(text, year) : year;
    case "number": {
      const digits = byRole.get("number");
      if (!digits || !text) return undefined;
      if (f.shape.kind === "slice") return digits.slice(f.shape.start, f.shape.start + f.shape.length) || undefined;
      return numberText(text, digits);
    }
    default:
      return byRole.get(f.kind);
  }
}

function announce(el: CardElement): void {
  el.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
}

function write(f: CardField, value: string): boolean {
  const el = f.el;
  if (el instanceof HTMLSelectElement) {
    if (f.kind !== "expiryMonth" && f.kind !== "expiryYear" && f.kind !== "brand") return false;
    const i = matchCardOption(el, f.kind, value);
    if (i < 0) return false;
    el.selectedIndex = i;
    written.set(el, String(i));
    announce(el);
    return true;
  }
  // Never cut a value to fit: a wrong card number or code is worse than an empty field.
  if (el.maxLength >= 0 && value.length > el.maxLength) return false;
  el.focus({ preventScroll: true });
  if (inputSetter) inputSetter.call(el, value);
  else el.value = value;
  written.set(el, value);
  announce(el);
  return true;
}

function mine(el: CardElement): boolean {
  const current = el instanceof HTMLSelectElement ? String(el.selectedIndex) : el.value;
  return written.get(el) === current;
}

function isEmpty(el: CardElement): boolean {
  if (el instanceof HTMLSelectElement) return el.selectedIndex <= 0 || el.value === "";
  return el.value === "";
}

function fillableNow(el: CardElement, env: Env): boolean {
  return el.isConnected && isCardFillable(el, env) && (isEmpty(el) || mine(el));
}

/** The roles worth asking for: those of fields we could write right now. */
export function cardRolesToFill(group: CardGroup, env: Env): CardRole[] {
  return cardRolesOf(group.fields.filter((f) => fillableNow(f.el, env)));
}

/** Fill the group's fields from `values`. Returns how many were written. */
export function fillCard(group: CardGroup, values: readonly CardValue[], env: Env): number {
  const byRole = new Map(values.map((v) => [v.role, v.value] as const));
  let n = 0;
  for (const f of group.fields) {
    const value = valueFor(f, byRole);
    if (value === undefined || !fillableNow(f.el, env)) continue;
    if (write(f, value)) n++;
  }
  return n;
}

// ------------------------------------------------------------ a typed card

export interface SubmittedCard {
  number: string;
  /** YYYY-MM. */
  expiry: string;
  verificationNumber: string | null;
  cardholderName: string | null;
}

const digitsOf = (v: string) => v.replace(/[\s.-]/g, "");

function monthOf(raw: string): number | null {
  const t = normalize(raw);
  if (/^\d{1,2}$/.test(t)) return Number(t);
  const i = MONTHS.findIndex((names) => t.split(" ").some((w) => names.includes(w)));
  return i >= 0 ? i + 1 : null;
}

function yearOf(raw: string): number | null {
  const t = raw.trim();
  if (/^\d{4}$/.test(t)) return Number(t);
  if (/^\d{2}$/.test(t)) return 2000 + Number(t);
  return null;
}

function selectedText(el: CardElement): string {
  if (el instanceof HTMLSelectElement) {
    const o = el.options[el.selectedIndex];
    return el.selectedIndex > 0 && o ? `${o.value || o.textContent || ""}` : "";
  }
  return el.value;
}

function readExpiry(group: CardGroup): string | null {
  const one = group.fields.find((f) => f.kind === "expiry");
  let month: number | null = null;
  let year: number | null = null;
  if (one) {
    const m = /^\s*(\d{1,2})\s*[/.-]?\s*(\d{2}|\d{4})\s*$/.exec(one.el.value);
    if (m) {
      month = Number(m[1]);
      year = yearOf(m[2] ?? "");
    }
  } else {
    const m = group.fields.find((f) => f.kind === "expiryMonth");
    const y = group.fields.find((f) => f.kind === "expiryYear");
    month = m ? monthOf(selectedText(m.el)) : null;
    year = y ? yearOf(selectedText(y.el)) : null;
  }
  if (month === null || year === null || month < 1 || month > 12 || year < 2000 || year > 2099) return null;
  return `${year}-${String(month).padStart(2, "0")}`;
}

/**
 * A card the user typed in `group`, worth offering to save. Only a number
 * the user typed counts (never one we filled or a page script wrote),
 * compared by digits so a site's mask does not hide it.
 */
export function readCardSubmission(group: CardGroup): SubmittedCard | null {
  const numberFields = group.fields.filter((f): f is CardField & { el: HTMLInputElement } => f.kind === "number" && f.el instanceof HTMLInputElement);
  if (numberFields.length === 0 || !numberFields.every((f) => typedByUser(f.el, digitsOf))) return null;
  const number = numberFields.map((f) => digitsOf(f.el.value)).join("");
  if (!/^[0-9]{12,19}$/.test(number) || !luhnOk(number)) return null;
  const expiry = readExpiry(group);
  if (!expiry) return null;
  const cvv = group.fields.find((f) => f.kind === "verificationNumber")?.el.value.trim() ?? "";
  const name = group.fields.find((f) => f.kind === "cardholderName")?.el.value.trim() ?? "";
  return {
    number,
    expiry,
    verificationNumber: /^[0-9]{3,8}$/.test(cvv) ? cvv : null,
    cardholderName: name.length > 0 && name.length <= 256 ? name : null,
  };
}
```

- [ ] **Step 5: Run the tests**

Run: `cd apps/extension && npx vitest run src/autofill/`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add apps/extension/src/autofill/card-fill.ts apps/extension/src/autofill/card-fill.test.ts apps/extension/src/autofill/fill.ts
git commit -m "feat(extension): write card values in each field's shape; read a typed card"
```

---

### Task 7: Extension messaging — card menu, frames, fill payload, save view, strings

**Files:**
- Modify: `apps/extension/src/messaging/inline.ts`
- Create: `apps/extension/src/messaging/inline-card.test.ts`
- Modify: `apps/extension/src/i18n/en.ts`, `apps/extension/src/i18n/pt-BR.ts`

**Interfaces:**
- Consumes: Task 4 (`CardRole`, `CardValue`, `CardBrandId`, `isCardRole`, `MAX_CARD_ROLES`, `MAX_CARD_VALUE_BYTES`) and Task 6 (`SubmittedCard`, re-declared here as a wire type of the same shape).
- Produces:
  - `MenuKind` gains `"card"`.
  - `interface Anchor { top: number; left: number; width: number; height: number }`.
  - `ContentRequest` gains three members:
    - `cs_open_menu` gets `cardRoles?: CardRole[]` and `anchor?: Anchor`;
    - `{ type: "cs_card_fields"; scan: string; roles: CardRole[] }`;
    - `{ type: "cs_card_submit"; card: SubmittedCardWire }`.
  - `OpenMenuReply`'s ok branch gains `hosted?: true`.
  - `FillPayload` gains `{ kind: "card"; values: CardValue[] }`.
  - `BackgroundToContent` gains `{ type: "bg_card_scan"; scan: string }` and `{ type: "bg_host_menu"; token: string; frameId: number; anchor: Anchor; rows: number }`.
  - `InlineRequest` gains `{ type: "menu_pick_card"; token: string; itemId: string }`.
  - `interface CardRowView { id; title; brand: CardBrandId | null; last4: string | null; expiry: string | null; expired: boolean }`.
  - `MenuView` gains `{ state: "cards"; site: string; cards: CardRowView[]; insecure: boolean }`.
  - `interface CardSaveView { site: string; title: string; card: { brand: CardBrandId | null; last4: string; expiry: string } }`.
  - `parseHostReply(v: unknown): boolean`.
  - New i18n keys:
    - `menu.cardFillOn`, `menu.cardRow`, `menu.cardExpired`, `menu.cardFallback`;
    - `menu.cardsInsecureTitle`, `menu.cardsInsecureBody`, `menu.noCardsTitle`, `menu.noCardsBody`;
    - `menu.cardNothing`, `menu.cardGone`;
    - `save.cardQuestion`;
    - `popup.cardsTitle`, `popup.fillCardTitle`;
    - `errors.noCardForm`.

- [ ] **Step 1: Write the failing tests** (`apps/extension/src/messaging/inline-card.test.ts`)

```ts
import { describe, expect, it } from "vitest";
import { parseBackgroundMessage, parseContentRequest, parseHostReply, parseInlineRequest } from "./inline";

const T = "a".repeat(32);
const ID = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const anchor = { top: 10, left: 20, width: 200, height: 30 };
const card = { number: "4111111111111111", expiry: "2033-04", verificationNumber: "123", cardholderName: "Samuel" };

describe("card messages", () => {
  it("accepts exact card content requests", () => {
    expect(parseContentRequest({ type: "cs_open_menu", kind: "card", cardRoles: ["number", "expiryMonth"] })).toEqual({
      type: "cs_open_menu",
      kind: "card",
      cardRoles: ["number", "expiryMonth"],
    });
    expect(parseContentRequest({ type: "cs_open_menu", kind: "card", cardRoles: ["number"], anchor })).toMatchObject({ anchor });
    expect(parseContentRequest({ type: "cs_card_fields", scan: T, roles: ["verificationNumber"] })).not.toBeNull();
    expect(parseContentRequest({ type: "cs_card_submit", card })).toEqual({ type: "cs_card_submit", card });
    expect(parseContentRequest({ type: "cs_card_submit", card: { ...card, verificationNumber: null, cardholderName: null } })).not.toBeNull();
  });

  it("rejects malformed card content requests", () => {
    for (const bad of [
      { type: "cs_open_menu", kind: "card" },
      { type: "cs_open_menu", kind: "card", cardRoles: [] },
      { type: "cs_open_menu", kind: "card", cardRoles: ["number", "number"] },
      { type: "cs_open_menu", kind: "card", cardRoles: ["password"] },
      { type: "cs_open_menu", kind: "card", cardRoles: ["number"], roles: ["fullName"] },
      { type: "cs_open_menu", kind: "login", cardRoles: ["number"] },
      { type: "cs_open_menu", kind: "login", anchor },
      { type: "cs_open_menu", kind: "card", cardRoles: ["number"], anchor: { ...anchor, width: -1 } },
      { type: "cs_open_menu", kind: "card", cardRoles: ["number"], anchor: { ...anchor, top: Infinity } },
      { type: "cs_card_fields", scan: "x", roles: ["number"] },
      { type: "cs_card_fields", scan: T, roles: [] },
      { type: "cs_card_submit", card: { ...card, number: "4111" } },
      { type: "cs_card_submit", card: { ...card, expiry: "04/33" } },
      { type: "cs_card_submit", card: { ...card, verificationNumber: "12" } },
      { type: "cs_card_submit", card: { ...card, cardholderName: "" } },
      { type: "cs_card_submit", card: { ...card, extra: 1 } },
    ]) {
      expect(parseContentRequest(bad), JSON.stringify(bad)).toBeNull();
    }
  });

  it("accepts card background messages; a card fill never submits", () => {
    const fill = { kind: "card", values: [{ role: "number", value: "4111111111111111" }] };
    expect(parseBackgroundMessage({ type: "bg_fill", origin: "https://a.com", token: null, fill, submit: false, totp: false })).not.toBeNull();
    expect(parseBackgroundMessage({ type: "bg_fill", origin: "https://a.com", token: null, fill, submit: true, totp: false })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_fill", origin: "https://a.com", token: null, fill: { kind: "card", values: [{ role: "number", value: "" }] }, submit: false, totp: false })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_card_scan", scan: T })).toEqual({ type: "bg_card_scan", scan: T });
    expect(parseBackgroundMessage({ type: "bg_host_menu", token: T, frameId: 3, anchor, rows: 2 })).not.toBeNull();
    expect(parseBackgroundMessage({ type: "bg_host_menu", token: T, frameId: 0, anchor, rows: 2 })).toBeNull();
    expect(parseBackgroundMessage({ type: "bg_host_menu", token: T, frameId: 3, anchor, rows: 9 })).toBeNull();
  });

  it("accepts menu_pick_card and host replies exactly", () => {
    expect(parseInlineRequest({ type: "menu_pick_card", token: T, itemId: ID })).toEqual({ type: "menu_pick_card", token: T, itemId: ID });
    expect(parseInlineRequest({ type: "menu_pick_card", token: T, itemId: "x" })).toBeNull();
    expect(parseHostReply({ ok: true })).toBe(true);
    expect(parseHostReply({ ok: false })).toBe(false);
    expect(parseHostReply(undefined)).toBe(false);
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/messaging/inline-card.test.ts`
Expected: FAIL; `parseHostReply` is not exported.

- [ ] **Step 3: Implement the types in `inline.ts`**

Extend the protocol import with `isCardRole, MAX_CARD_ROLES, MAX_CARD_VALUE_BYTES, type CardBrandId, type CardRole, type CardValue`. Then change the types:

```ts
export type MenuKind = "login" | "otp" | "new_password" | "identity" | "card";

/** A field's box in its own frame's viewport (card menus a processor frame asks the top frame to host). */
export interface Anchor {
  top: number;
  left: number;
  width: number;
  height: number;
}

/** A card the user typed into a checkout (autofill/card-fill.ts readCardSubmission). */
export interface SubmittedCardWire {
  number: string;
  /** YYYY-MM. */
  expiry: string;
  verificationNumber: string | null;
  cardholderName: string | null;
}
```

- In `ContentRequest`, change the `cs_open_menu` member to `{ type: "cs_open_menu"; kind: MenuKind; explicit?: true; roles?: IdentityRole[]; cardRoles?: CardRole[]; anchor?: Anchor }`, and add these members:

```ts
  /** This frame's card fields, answering bg_card_scan. */
  | { type: "cs_card_fields"; scan: string; roles: CardRole[] }
  /** The user submitted a card they typed (top frame only). */
  | { type: "cs_card_submit"; card: SubmittedCardWire }
```

- `OpenMenuReply` becomes `{ ok: true; token: string; rows: number; hosted?: true } | { ok: false }`.
- `FillPayload` gains `| { kind: "card"; values: CardValue[] }`.
- `BackgroundToContent` gains these members:

```ts
  /** Every frame: report your card fields with cs_card_fields. */
  | { type: "bg_card_scan"; scan: string }
  /** Top frame: show menu `token` over child frame `frameId`, at `anchor` inside it. Reply { ok }. */
  | { type: "bg_host_menu"; token: string; frameId: number; anchor: Anchor; rows: number }
```

- `InlineRequest` gains `| { type: "menu_pick_card"; token: string; itemId: string }`.
- Add these views:

```ts
/** A card row in the menu. No number beyond the last four digits. */
export interface CardRowView {
  id: string;
  title: string;
  brand: CardBrandId | null;
  last4: string | null;
  /** MM/YY. */
  expiry: string | null;
  expired: boolean;
}

/** The card save prompt. */
export interface CardSaveView {
  site: string;
  /** The suggested name: the brand's. */
  title: string;
  card: { brand: CardBrandId | null; last4: string; expiry: string };
}
```

- `MenuView` gains `| { state: "cards"; site: string; cards: CardRowView[]; insecure: boolean }`.

- [ ] **Step 4: Implement the parsers in `inline.ts`**
  - `MENU_KINDS` gains `"card"`.
  - Add these helpers after `parseRoleList`:

```ts
function parseCardRoleList(v: unknown): CardRole[] | null {
  if (!Array.isArray(v) || v.length === 0 || v.length > MAX_CARD_ROLES) return null;
  const out: CardRole[] = [];
  for (const r of v) {
    if (!isCardRole(r) || out.includes(r)) return null;
    out.push(r);
  }
  return out;
}

const within = (x: unknown, lo: number, hi: number): x is number => typeof x === "number" && Number.isFinite(x) && x >= lo && x <= hi;

function parseAnchor(v: unknown): Anchor | null {
  const o = obj(v);
  if (!o || !keysAre(o, ["top", "left", "width", "height"])) return null;
  const { top, left, width, height } = o;
  return within(top, -1e5, 1e5) && within(left, -1e5, 1e5) && within(width, 0, 1e4) && within(height, 0, 1e4) ? { top, left, width, height } : null;
}

function parseSubmittedCard(v: unknown): SubmittedCardWire | null {
  const o = obj(v);
  if (!o || !keysAre(o, ["number", "expiry", "verificationNumber", "cardholderName"])) return null;
  const { number, expiry, verificationNumber, cardholderName } = o;
  if (typeof number !== "string" || !/^[0-9]{12,19}$/.test(number)) return null;
  if (typeof expiry !== "string" || !/^\d{4}-(0[1-9]|1[0-2])$/.test(expiry)) return null;
  if (verificationNumber !== null && !(typeof verificationNumber === "string" && /^[0-9]{3,8}$/.test(verificationNumber))) return null;
  if (!boundedOrNull(cardholderName, 256)) return null;
  return { number, expiry, verificationNumber, cardholderName };
}

/** The top frame's answer to bg_host_menu. */
export function parseHostReply(v: unknown): boolean {
  const o = obj(v);
  return !!o && keysAre(o, ["ok"]) && o.ok === true;
}
```

  - Replace the `cs_open_menu` case of `parseContentRequest`:

```ts
    case "cs_open_menu": {
      if (!MENU_KINDS.includes(o.kind as MenuKind)) return null;
      const kind = o.kind as MenuKind;
      const keys = Object.keys(o).filter((k) => k !== "type" && k !== "kind");
      if (keys.some((k) => k !== "explicit" && k !== "roles" && k !== "cardRoles" && k !== "anchor")) return null;
      if ("explicit" in o && o.explicit !== true) return null;
      let roles: IdentityRole[] | undefined;
      if ("roles" in o) {
        if (kind !== "identity" && kind !== "login") return null;
        const parsed = parseRoleList(o.roles);
        if (!parsed) return null;
        roles = parsed;
      } else if (kind === "identity") return null;
      let cardRoles: CardRole[] | undefined;
      let anchor: Anchor | undefined;
      if (kind === "card") {
        const parsed = parseCardRoleList(o.cardRoles);
        if (!parsed) return null;
        cardRoles = parsed;
        if ("anchor" in o) {
          const a = parseAnchor(o.anchor);
          if (!a) return null;
          anchor = a;
        }
      } else if ("cardRoles" in o || "anchor" in o) return null;
      return {
        type: "cs_open_menu",
        kind,
        ...(o.explicit === true ? { explicit: true as const } : {}),
        ...(roles ? { roles } : {}),
        ...(cardRoles ? { cardRoles } : {}),
        ...(anchor ? { anchor } : {}),
      };
    }
```

  - Add these cases to `parseContentRequest`:

```ts
    case "cs_card_fields": {
      if (!keysAre(o, ["type", "scan", "roles"]) || !isToken(o.scan)) return null;
      const roles = parseCardRoleList(o.roles);
      return roles && { type: "cs_card_fields", scan: o.scan, roles };
    }
    case "cs_card_submit": {
      if (!keysAre(o, ["type", "card"])) return null;
      const card = parseSubmittedCard(o.card);
      return card && { type: "cs_card_submit", card };
    }
```

  - Add this case to `parseInlineRequest`:

```ts
    case "menu_pick_card":
      return keysAre(o, ["type", "token", "itemId"]) && isUuid(o.itemId) ? { type: "menu_pick_card", token, itemId: o.itemId } : null;
```

  - Add this case to `parseFill`:

```ts
    case "card": {
      if (!keysAre(o, ["kind", "values"]) || !Array.isArray(o.values) || o.values.length > MAX_CARD_ROLES) return null;
      const values: CardValue[] = [];
      for (const x of o.values) {
        const v = obj(x);
        if (!v || !keysAre(v, ["role", "value"]) || !isCardRole(v.role) || typeof v.value !== "string") return null;
        const role: CardRole = v.role;
        if (v.value.length === 0 || v.value.length > MAX_CARD_VALUE_BYTES || values.some((y) => y.role === role)) return null;
        values.push({ role, value: v.value });
      }
      return { kind: "card", values };
    }
```

  - In `parseBackgroundMessage`, in `bg_fill`, change the guard to `if ((fill?.kind === "identity" || fill?.kind === "card") && (o.submit || o.totp)) return null;` and add these cases:

```ts
    case "bg_card_scan":
      return keysAre(o, ["type", "scan"]) && isToken(o.scan) ? { type: "bg_card_scan", scan: o.scan } : null;
    case "bg_host_menu": {
      if (!keysAre(o, ["type", "token", "frameId", "anchor", "rows"]) || !isToken(o.token)) return null;
      const anchor = parseAnchor(o.anchor);
      const { frameId, rows } = o;
      if (!anchor || typeof frameId !== "number" || !Number.isInteger(frameId) || frameId < 1) return null;
      if (typeof rows !== "number" || !Number.isInteger(rows) || rows < 1 || rows > MENU_MAX_ROWS) return null;
      return { type: "bg_host_menu", token: o.token, frameId, anchor, rows };
    }
```

- [ ] **Step 5: Add the strings**

The `Messages` type makes both files required. In `en.ts`, add to `menu`:

```ts
    cardFillOn: (site: string) => `Fill on ${site}`,
    cardRow: (last4: string | null, expiry: string | null) => [last4 ? `•••• ${last4}` : null, expiry].filter(Boolean).join(" · "),
    cardExpired: "Expired",
    cardFallback: "Card",
    cardsInsecureTitle: "Not a secure page",
    cardsInsecureBody: "HavenKeys fills cards only on secure (https) pages.",
    noCardsTitle: "No cards saved",
    noCardsBody: "Add one in the HavenKeys app.",
    cardNothing: "Nothing to fill in this form.",
    cardGone: "This card is no longer in HavenKeys.",
```

- To `save`: `cardQuestion: "Save this card to HavenKeys?",`.
- To `popup`: `cardsTitle: "Cards", fillCardTitle: "Fill this card into the page",`.
- To `errors`: `noCardForm: "No card form found on this page.",`.

In `pt-BR.ts`, add the same keys:

```ts
    cardFillOn: (site: string) => `Preencher em ${site}`,
    cardRow: (last4: string | null, expiry: string | null) => [last4 ? `•••• ${last4}` : null, expiry].filter(Boolean).join(" · "),
    cardExpired: "Vencido",
    cardFallback: "Cartão",
    cardsInsecureTitle: "Página não segura",
    cardsInsecureBody: "O HavenKeys só preenche cartões em páginas seguras (https).",
    noCardsTitle: "Nenhum cartão salvo",
    noCardsBody: "Adicione um no app HavenKeys.",
    cardNothing: "Nada para preencher neste formulário.",
    cardGone: "Este cartão não está mais no HavenKeys.",
```

- To `save`: `cardQuestion: "Salvar este cartão no HavenKeys?",`.
- To `popup`: `cardsTitle: "Cartões", fillCardTitle: "Preencher este cartão na página",`.
- To `errors`: `noCardForm: "Nenhum formulário de cartão nesta página.",`.

- [ ] **Step 6: Run the tests and typecheck**

Run: `cd apps/extension && npx vitest run src/messaging src/i18n src/background/inline-handler.test.ts && npx tsc --noEmit -p .`
Expected: all pass. The typecheck may flag an exhaustive `switch` over `FillPayload` or `BackgroundToContent` in `content/index.ts`. If so, add a temporary `case "card": return none;` there (`handleFill`) and `case "bg_card_scan": case "bg_host_menu": return false;` (listener); Task 9 replaces both.

- [ ] **Step 7: Commit**

```bash
git add apps/extension/src/messaging apps/extension/src/i18n apps/extension/src/content/index.ts
git commit -m "feat(extension): card menu, scan, host and fill messages, strictly parsed; card strings"
```

---

### Task 8: Background — card menu session, multi-frame fill, hosted menus, save prompt

**Files:**
- Create: `apps/extension/src/background/card-rows.ts`
- Modify: `apps/extension/src/background/inline-handler.ts`
- Modify: `apps/extension/src/background/index.ts`
- Test: `apps/extension/src/background/inline-card.test.ts`

**Interfaces:**
- Consumes: Tasks 4 and 7.
- Produces:
  - `card-rows.ts`: `displayExpiry(iso: string | null): string | null`, `isExpired(iso: string | null, now: number): boolean`, `cardRows(cards: readonly CardMatch[], now: number): CardRowView[]`.
  - `InlineDeps` gains `sendToTab?(tabId: number, msg: BackgroundToContent): Promise<unknown>` and `wait?(ms: number): Promise<void>`.
  - The handler returns `fillCard(tabId: number, topUrl: string, clicked: CardTarget | null, itemId: string): Promise<number>` and `scanCards(tabId: number): Promise<CardReport[]>`, with `interface CardTarget { frame: FrameRef; token: string | null; roles: CardRole[] }` and `interface CardReport { frame: FrameRef; roles: CardRole[] }`.
  - Constants: `CARD_SCAN_MS = 300`, `CARD_SAVE_TTL_MS = 120_000`, `MAX_SCAN_REPORTS = 16`.

- [ ] **Step 1: Write the failing tests** (`apps/extension/src/background/inline-card.test.ts`)

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Request } from "@havenkeys/protocol";
import { BridgeError } from "../messaging/native";
import type { BackgroundToContent } from "../messaging/inline";
import { cardRows, displayExpiry } from "./card-rows";
import { createInlineHandler, type CardReport, type FrameRef } from "./inline-handler";

const VISA = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const OLD = "11111111-2222-4333-8444-555555555555";
const SHOP = "https://shop.com/checkout";
const top: FrameRef = { tabId: 1, frameId: 0, url: SHOP, origin: "https://shop.com" };
const stripe = (frameId: number, topUrl = SHOP): FrameRef => ({
  tabId: 1,
  frameId,
  url: `https://js.stripe.com/v3/elements-inner-${frameId}.html`,
  topUrl,
  origin: "https://js.stripe.com",
});
const cards = [
  { id: OLD, title: "Old Visa", brand: "visa" as const, last4: "0004", expiry: "2020-01" },
  { id: VISA, title: "Visa", brand: "visa" as const, last4: "1111", expiry: "2033-04" },
];

function setup(answer: (r: Request) => unknown, reports: CardReport[] = []) {
  const requests: Request[] = [];
  const sent: Array<{ frameId: number; msg: BackgroundToContent }> = [];
  let n = 0;
  const h = createInlineHandler({
    client: {
      request: (async (r: Request) => {
        requests.push(r);
        const a = answer(r);
        if (a instanceof Error) throw a;
        return a;
      }) as never,
    },
    sendToFrame: async (to, msg) => {
      sent.push({ frameId: to.frameId, msg: msg as BackgroundToContent });
      if ((msg as BackgroundToContent).type === "bg_fill") return { filled: 1, pressing: null };
      if ((msg as BackgroundToContent).type === "bg_host_menu") return { ok: true };
      return undefined;
    },
    sendToTab: async (_tabId, msg) => {
      if (msg.type !== "bg_card_scan") return;
      for (const r of reports) void h.handleContent(r.frame, { type: "cs_card_fields", scan: msg.scan, roles: r.roles });
    },
    wait: async () => {},
    now: () => Date.UTC(2026, 8, 29),
    newToken: () => (++n).toString(16).padStart(32, "0"),
  });
  return { h, requests, sent };
}

const findCards = (r: Request) =>
  r.type === "find_cards"
    ? { type: "find_cards", insecure: !(r.topUrl ?? r.url).startsWith("https:"), cards: r.url.includes("ads.") ? [] : cards }
    : r.type === "fill_card"
      ? { type: "fill_card", frames: r.frames.map(() => ({ values: [{ role: "number", value: "4111111111111111" }] })) }
      : new Error(`unexpected ${r.type}`);

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("card rows", () => {
  it("shows MM/YY and puts expired cards last", () => {
    expect(displayExpiry("2033-04")).toBe("04/33");
    expect(cardRows(cards, Date.UTC(2026, 8, 29)).map((c) => [c.title, c.expiry, c.expired])).toEqual([
      ["Visa", "04/33", false],
      ["Old Visa", "01/20", true],
    ]);
  });
});

describe("card menu", () => {
  it("offers the cards with the top page's site, never numbers", async () => {
    const { h } = setup(findCards);
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })) as { ok: boolean; token: string; rows: number };
    expect(open).toMatchObject({ ok: true, rows: 2 });
    const view = await h.handleInline(1, { type: "menu_state", token: open.token });
    expect(view).toEqual({
      ok: true,
      value: {
        state: "cards",
        site: "shop.com",
        insecure: false,
        cards: [
          { id: VISA, title: "Visa", brand: "visa", last4: "1111", expiry: "04/33", expired: false },
          { id: OLD, title: "Old Visa", brand: "visa", last4: "0004", expiry: "01/20", expired: true },
        ],
      },
    });
  });

  it("explains an http page instead of filling", async () => {
    const { h } = setup(findCards);
    const open = (await h.handleContent({ ...top, url: "http://shop.com/", origin: "http://shop.com" }, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })) as { token: string };
    expect(await h.handleInline(1, { type: "menu_state", token: open.token })).toMatchObject({ value: { state: "cards", insecure: true, cards: [] } });
  });

  it("shows the unlock row when locked, and nothing in a frame Rust denies", async () => {
    const locked = setup(() => new BridgeError("locked", "x"));
    const open = (await locked.h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })) as { token: string };
    expect(await locked.h.handleInline(1, { type: "menu_state", token: open.token })).toEqual({ ok: true, value: { state: "locked" } });
    const denied = setup(() => new BridgeError("denied", "x"));
    expect(await denied.h.handleContent(stripe(3), { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })).toEqual({ ok: false });
  });

  it("asks the top frame to host a menu opened in a processor frame", async () => {
    const { h, sent } = setup(findCards);
    const anchor = { top: 5, left: 5, width: 200, height: 30 };
    const open = await h.handleContent(stripe(3), { type: "cs_open_menu", kind: "card", cardRoles: ["number"], anchor });
    expect(open).toMatchObject({ ok: true, hosted: true });
    expect(sent.at(-1)).toMatchObject({ frameId: 0, msg: { type: "bg_host_menu", frameId: 3, anchor, rows: 2 } });
    // Closing reaches both the frame and the host.
    await h.handleInline(1, { type: "menu_close", token: (open as { token: string }).token });
    expect(sent.filter((s) => s.msg.type === "bg_close_menu").map((s) => s.frameId).sort()).toEqual([0, 3]);
  });
});

describe("card pick", () => {
  it("fills the clicked frame and the tab's other card frames with one fill_card", async () => {
    const { h, requests, sent } = setup(findCards, [
      { frame: stripe(3), roles: ["number"] },
      { frame: stripe(4), roles: ["expiryMonth", "expiryYear"] },
    ]);
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["cardholderName"] })) as { token: string };
    expect(await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: VISA })).toEqual({ ok: true, value: null });
    const fill = requests.find((r) => r.type === "fill_card");
    expect(fill).toEqual({
      type: "fill_card",
      itemId: VISA,
      topUrl: SHOP,
      frames: [
        { url: SHOP, roles: ["cardholderName"] },
        { url: stripe(3).url, roles: ["number"] },
        { url: stripe(4).url, roles: ["expiryMonth", "expiryYear"] },
      ],
    });
    const fills = sent.filter((s) => s.msg.type === "bg_fill");
    expect(fills.map((s) => [s.frameId, (s.msg as { token: string | null }).token])).toEqual([[0, open.token], [3, null], [4, null]]);
    expect(fills.every((s) => (s.msg as { submit: boolean }).submit === false)).toBe(true);
  });

  it("leaves out a frame from another top page and a frame Rust denies", async () => {
    const ads: FrameRef = { tabId: 1, frameId: 5, url: "https://ads.example.net/x", topUrl: SHOP, origin: "https://ads.example.net" };
    const { h, requests } = setup(
      (r) => (r.type === "find_cards" && r.url.startsWith("https://ads.") ? new BridgeError("denied", "x") : findCards(r)),
      [
        { frame: stripe(3, "https://other.com/"), roles: ["number"] },
        { frame: ads, roles: ["number"] },
      ],
    );
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })) as { token: string };
    await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: VISA });
    expect(requests.find((r) => r.type === "fill_card")).toMatchObject({ frames: [{ url: SHOP }] });
  });

  it("caps the frames", async () => {
    const reports = Array.from({ length: 20 }, (_, i) => ({ frame: stripe(i + 1), roles: ["number" as const] }));
    const { h, requests } = setup(findCards, reports);
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["cardholderName"] })) as { token: string };
    await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: VISA });
    const fill = requests.find((r) => r.type === "fill_card") as Extract<Request, { type: "fill_card" }>;
    expect(fill.frames).toHaveLength(8);
    // At most 16 reports were kept, so at most 7 extra frames were looked up.
    expect(requests.filter((r) => r.type === "find_cards").length).toBeLessThanOrEqual(1 + 16);
  });

  it("refuses a card the menu never offered, and says so when a card was deleted", async () => {
    const { h } = setup((r) => (r.type === "fill_card" ? new BridgeError("not_found", "x") : findCards(r)));
    const open = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })) as { token: string };
    expect(await h.handleInline(1, { type: "menu_pick_card", token: open.token, itemId: "22222222-2222-4222-8222-222222222222" })).toMatchObject({ ok: false });
    const again = (await h.handleContent(top, { type: "cs_open_menu", kind: "card", cardRoles: ["number"] })) as { token: string };
    expect(await h.handleInline(1, { type: "menu_pick_card", token: again.token, itemId: VISA })).toEqual({ ok: false, message: "This card is no longer in HavenKeys." });
  });
});

describe("card save prompt", () => {
  const typed = { number: "4000056655665556", expiry: "2030-01", verificationNumber: "123", cardholderName: "Samuel" };

  it("offers a new card in the top frame and saves it on confirm", async () => {
    const { h, requests, sent } = setup((r) => (r.type === "save_card" ? { type: "save_card", itemId: VISA } : findCards(r)));
    await h.handleContent(top, { type: "cs_card_submit", card: typed });
    const show = sent.find((s) => s.msg.type === "bg_show_save")!;
    const token = (show.msg as { token: string }).token;
    expect(await h.handleInline(1, { type: "save_state", token })).toEqual({
      ok: true,
      value: { site: "shop.com", title: "Visa", card: { brand: "visa", last4: "5556", expiry: "01/30" } },
    });
    expect(await h.handleInline(1, { type: "save_confirm", token, title: "Meu Visa" })).toEqual({ ok: true, value: null });
    expect(requests.at(-1)).toEqual({ type: "save_card", url: SHOP, title: "Meu Visa", number: typed.number, expiry: "2030-01", verificationNumber: "123", cardholderName: "Samuel" });
  });

  it("stays quiet for a saved card, an iframe, http and a failing number", async () => {
    const { h, sent } = setup(findCards);
    await h.handleContent(top, { type: "cs_card_submit", card: { ...typed, number: "4111111111111111", expiry: "2033-04" } }); // same last 4 and expiry as VISA
    await h.handleContent(stripe(3), { type: "cs_card_submit", card: typed });
    await h.handleContent({ ...top, url: "http://shop.com/", origin: "http://shop.com" }, { type: "cs_card_submit", card: typed });
    await h.handleContent(top, { type: "cs_card_submit", card: { ...typed, number: "4000056655665557" } });
    expect(sent.filter((s) => s.msg.type === "bg_show_save")).toEqual([]);
  });

  it("drops the typed card after two minutes and on lock", async () => {
    const { h } = setup(findCards);
    await h.handleContent(top, { type: "cs_card_submit", card: typed });
    expect(await h.handleContent(top, { type: "cs_ready" })).toMatchObject({ saveToken: expect.any(String) });
    vi.advanceTimersByTime(120_001);
    expect(await h.handleContent(top, { type: "cs_ready" })).toMatchObject({ saveToken: null });
    await h.handleContent(top, { type: "cs_card_submit", card: typed });
    h.reset();
    expect(await h.handleContent(top, { type: "cs_ready" })).toMatchObject({ saveToken: null });
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/background/inline-card.test.ts`
Expected: FAIL; `./card-rows` does not exist.

- [ ] **Step 3: Create `apps/extension/src/background/card-rows.ts`**

```ts
// Card rows for the menu and popup: display expiry and expired-last order.
// Overview data only (find_cards); nothing here sees a number.

import type { CardMatch } from "@havenkeys/protocol";
import type { CardRowView } from "../messaging/inline";

/** "2033-04" → "04/33". */
export function displayExpiry(iso: string | null): string | null {
  const m = iso ? /^(\d{4})-(\d{2})$/.exec(iso) : null;
  return m ? `${m[2]}/${(m[1] as string).slice(2)}` : null;
}

/** The card's last valid month has passed (local time). */
export function isExpired(iso: string | null, now: number): boolean {
  if (!iso) return false;
  const d = new Date(now);
  return iso < `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

export function cardRows(cards: readonly CardMatch[], now: number): CardRowView[] {
  const rows = cards.map((c) => ({
    id: c.id,
    title: c.title,
    brand: c.brand,
    last4: c.last4,
    expiry: displayExpiry(c.expiry),
    expired: isExpired(c.expiry, now),
  }));
  return [...rows.filter((r) => !r.expired), ...rows.filter((r) => r.expired)];
}
```

- [ ] **Step 4: Change `inline-handler.ts`**
  - **Imports:**
    - Add `brandOf, CARD_BRAND_NAMES, luhnOk, MAX_CARD_FRAMES, type CardBrandId, type CardRole` from `@havenkeys/protocol`.
    - Add `parseHostReply, type Anchor, type CardRowView, type CardSaveView, type SubmittedCardWire` from `../messaging/inline`.
    - Add `cardRows, displayExpiry` from `./card-rows`.
  - **Header comment:** add a bullet:

```ts
// * Cards are not site-bound (spec 2026-09-29-card-autofill §2): a pick
//   fills the clicked frame and the tab's other card frames that Rust
//   accepts (find_cards per frame, then one fill_card that Rust re-checks
//   frame by frame). Typed cards wait for the user's save in memory only,
//   for at most CARD_SAVE_TTL_MS.
```

  - **`InlineDeps`:** add these members:

```ts
  /** Send to every frame of a tab (the card scan). */
  sendToTab?(tabId: number, msg: BackgroundToContent): Promise<unknown>;
  /** Wait `ms` (the card scan's window). Absent: setTimeout. */
  wait?(ms: number): Promise<void>;
```

  - **Exported constants and types:**

```ts
export const CARD_SCAN_MS = 300;
export const CARD_SAVE_TTL_MS = 120_000;
export const MAX_SCAN_REPORTS = 16;

/** A frame's card fields, as it reported them (cs_card_fields). */
export interface CardReport {
  frame: FrameRef;
  roles: CardRole[];
}

/** A frame to fill: its roles, and the menu token for the frame the user clicked in. */
export interface CardTarget {
  frame: FrameRef;
  token: string | null;
  roles: CardRole[];
}

interface CardMenu {
  roles: CardRole[];
  rows: CardRowView[];
  insecure: boolean;
}

interface PendingCardSave {
  token: string;
  frame: FrameRef;
  card: SubmittedCardWire;
  brand: CardBrandId | null;
  title: string;
  expires: number;
}
```

  - **`MenuSession`:** add `card: CardMenu | null;` and `/** The top frame showing this menu for a processor frame. */ host: Pick<FrameRef, "tabId" | "frameId"> | null;`.
  - **`register`:** give it a last parameter `card: CardMenu | null = null`. The session becomes `{ …, card, host: null, expires }` and rows are computed as:

```ts
    const rows =
      locked || kind === "new_password" || kind === "identity"
        ? 1
        : kind === "card"
          ? Math.max(1, Math.min(card && !card.insecure ? card.rows.length : 1, MENU_MAX_ROWS))
          : Math.max(1, Math.min(offered, MENU_MAX_ROWS));
```

  - **State:** inside `createInlineHandler`, add `const cardSaves = new Map<number, PendingCardSave>();` and `const scans = new Map<string, { tabId: number; reports: CardReport[] }>();`.
  - **`closeMenu`:** also notify the host:

```ts
  function closeMenu(tabId: number): void {
    const m = menus.get(tabId);
    if (!m) return;
    menus.delete(tabId);
    void deps.sendToFrame(m.frame, { type: "bg_close_menu", token: m.token });
    if (m.host) void deps.sendToFrame(m.host, { type: "bg_close_menu", token: m.token });
  }
```

  - **Card saves:** add `dropCardSave` and `liveCardSave` next to `dropSave` and `liveSave`:

```ts
  function dropCardSave(tabId: number, notify: boolean): void {
    const s = cardSaves.get(tabId);
    if (!s) return;
    cardSaves.delete(tabId);
    // Strings cannot be wiped in JS; dropping the only reference is the most we can do.
    s.card = { number: "", expiry: "", verificationNumber: null, cardholderName: null };
    if (notify) void deps.sendToFrame(top(s.frame), { type: "bg_close_save", token: s.token });
  }

  function liveCardSave(tabId: number, token: string): PendingCardSave | null {
    const s = cardSaves.get(tabId);
    if (!s || s.token !== token) return null;
    if (s.expires <= deps.now()) {
      dropCardSave(tabId, true);
      return null;
    }
    return s;
  }
```

  - **Card menu, scan and fill:** add these functions after `identityRow`:

```ts
  /** Ask the top frame to show a processor frame's menu over it (the frame itself is too small). */
  async function hostMenu(reply: OpenMenuReply, frame: FrameRef, anchor: Anchor | null): Promise<OpenMenuReply> {
    if (!reply.ok || frame.frameId === 0 || !anchor) return reply;
    const m = menus.get(frame.tabId);
    if (!m || m.token !== reply.token) return reply;
    const r = await deps.sendToFrame(top(frame), { type: "bg_host_menu", token: reply.token, frameId: frame.frameId, anchor, rows: reply.rows });
    if (!parseHostReply(r) || menus.get(frame.tabId) !== m) return reply;
    m.host = top(frame);
    return { ...reply, hosted: true };
  }

  async function openCardMenu(frame: FrameRef, roles: CardRole[], anchor: Anchor | null): Promise<OpenMenuReply> {
    let list: ResultFor<"find_cards">;
    try {
      list = await deps.client.request({ type: "find_cards", ...frameFields(frame) });
    } catch (e) {
      if (e instanceof BridgeError && e.code === "locked") {
        return hostMenu(register(frame, "card", true, [], [], { hint: null, help: null }, [], null, { roles, rows: [], insecure: false }), frame, anchor);
      }
      // A frame Rust denies, integration off, app gone: stay out of the page.
      return { ok: false };
    }
    const card = { roles, rows: cardRows(list.cards, deps.now()), insecure: list.insecure };
    return hostMenu(register(frame, "card", false, [], [], { hint: null, help: null }, [], null, card), frame, anchor);
  }

  /** Every frame of the tab reports its card fields for CARD_SCAN_MS. */
  async function scanCards(tabId: number): Promise<CardReport[]> {
    if (!deps.sendToTab) return [];
    const scan = deps.newToken();
    scans.set(scan, { tabId, reports: [] });
    void deps.sendToTab(tabId, { type: "bg_card_scan", scan });
    await (deps.wait ?? ((ms: number) => new Promise<void>((r) => setTimeout(r, ms))))(CARD_SCAN_MS);
    const s = scans.get(scan);
    scans.delete(scan);
    return s?.reports ?? [];
  }

  /** Rust would serve this frame a card (a lookup: no values). */
  async function cardFrameAllowed(frame: FrameRef): Promise<boolean> {
    try {
      return !(await deps.client.request({ type: "find_cards", ...frameFields(frame) })).insecure;
    } catch {
      return false;
    }
  }

  /**
   * Fill a card into the clicked frame (if any) and the tab's other card
   * frames that are on the same top page and that Rust accepts. One
   * fill_card; each frame gets its own values, pinned to its document.
   */
  async function fillCard(tabId: number, topUrl: string, clicked: CardTarget | null, itemId: string): Promise<number> {
    endRun(tabId);
    const reports = await scanCards(tabId);
    const targets: CardTarget[] = clicked && clicked.roles.length > 0 ? [clicked] : [];
    for (const r of reports) {
      if (targets.length >= MAX_CARD_FRAMES) break;
      if (targets.some((t) => t.frame.frameId === r.frame.frameId)) continue;
      if ((r.frame.topUrl ?? r.frame.url) !== topUrl) continue;
      if (!(await cardFrameAllowed(r.frame))) continue;
      targets.push({ frame: r.frame, token: null, roles: r.roles });
    }
    if (targets.length === 0) return 0;
    const res = await deps.client.request({ type: "fill_card", itemId, topUrl, frames: targets.map((t) => ({ url: t.frame.url, roles: t.roles })) });
    let n = 0;
    for (const [i, t] of targets.entries()) {
      const values = res.frames[i]?.values ?? [];
      if (values.length > 0) n += (await sendFill(t.frame, t.token, { kind: "card", values }, false, false)).filled;
    }
    return n;
  }

  /** A card the user typed in the top frame: ask to save it unless it is already saved. */
  async function submitCard(frame: FrameRef, card: SubmittedCardWire): Promise<void> {
    if (frame.frameId !== 0 || !frame.url.startsWith("https:") || !luhnOk(card.number)) return;
    let list: ResultFor<"find_cards">;
    try {
      list = await deps.client.request({ type: "find_cards", url: frame.url });
    } catch {
      return;
    }
    if (list.insecure) return;
    const last4 = card.number.slice(-4);
    if (list.cards.some((c) => c.last4 === last4 && c.expiry === card.expiry)) return;
    dropSave(frame.tabId, true);
    dropCardSave(frame.tabId, true);
    const brand = brandOf(card.number);
    const token = deps.newToken();
    cardSaves.set(frame.tabId, {
      token,
      frame,
      card,
      brand,
      title: brand && brand !== "other" ? CARD_BRAND_NAMES[brand] : t.menu.cardFallback,
      expires: deps.now() + CARD_SAVE_TTL_MS,
    });
    setTimeout(() => {
      if (cardSaves.get(frame.tabId)?.token === token) dropCardSave(frame.tabId, true);
    }, CARD_SAVE_TTL_MS);
    void deps.sendToFrame(top(frame), { type: "bg_show_save", token });
  }
```

  - **`openMenu`:** give it two more parameters, `cardRoles: CardRole[] = [], anchor: Anchor | null = null`. Right after the `suggestionsOn` check (before `if (kind === "identity")`), add `if (kind === "card") return openCardMenu(frame, cardRoles, anchor);`. In the suggestions-off branch, `kind === "card"` returns `{ ok: false }` as the code already does for anything but login.
  - **`submit` (login):** add `dropCardSave(frame.tabId, true);` next to `dropSave(frame.tabId, true);`, so there is one prompt per tab.
  - **`ready`:** a card save is offered like a login save:

```ts
    const s = saves.get(frame.tabId);
    const c = cardSaves.get(frame.tabId);
    if (c && c.expires > deps.now()) return { saveToken: c.token, watch };
    if (!s || s.expires <= deps.now()) {
      dropSave(frame.tabId, false);
      dropCardSave(frame.tabId, false);
      return { saveToken: null, watch };
    }
    return { saveToken: s.token, watch };
```

  - **`handleContent`:** pass the new fields through: `openMenu(frame, req.kind, req.explicit === true, req.roles ?? [], req.cardRoles ?? [], req.anchor ?? null)`. In `cs_close_menu`, also accept the host frame: `if (m && m.token === req.token && (m.frame.frameId === frame.frameId || m.host?.frameId === frame.frameId)) closeMenu(frame.tabId);`. Then add:

```ts
      case "cs_card_fields": {
        const s = scans.get(req.scan);
        if (s && s.tabId === frame.tabId && s.reports.length < MAX_SCAN_REPORTS && !s.reports.some((r) => r.frame.frameId === frame.frameId)) {
          s.reports.push({ frame, roles: req.roles });
        }
        return {};
      }
      case "cs_card_submit":
        await submitCard(frame, req.card);
        return {};
```

  - **`handleInline`:**
    - The return type becomes `Promise<InlineReply<MenuView | SaveView | CardSaveView | null>>`.
    - In `menu_state`, after the locked check: `if (m.kind === "card" && m.card) return { ok: true, value: { state: "cards", site: displayHost(m.frame.topUrl ?? m.frame.url) ?? "", cards: m.card.insecure ? [] : m.card.rows, insecure: m.card.insecure } };`.
    - In `menu_resize`, send to `m.host ?? m.frame`.
    - Add the pick:

```ts
      case "menu_pick_card": {
        const m = liveMenu(tabId, req.token);
        if (!m || m.locked || m.kind !== "card" || !m.card) return { ok: false, message: t.errors.menuExpired };
        if (!m.card.rows.some((c) => c.id === req.itemId)) return { ok: false, message: t.errors.unknownItem };
        closeMenu(tabId);
        try {
          const filled = await fillCard(tabId, m.frame.topUrl ?? m.frame.url, { frame: m.frame, token: m.token, roles: m.card.roles }, req.itemId);
          return filled > 0 ? { ok: true, value: null } : { ok: false, message: t.menu.cardNothing };
        } catch (e) {
          if (e instanceof BridgeError && e.code === "not_found") return { ok: false, message: t.menu.cardGone };
          return fail(e);
        }
      }
```

    - `save_state`, `save_confirm`, `save_dismiss` and `save_resize` check the card save first:

```ts
      case "save_state": {
        const c = liveCardSave(tabId, req.token);
        if (c) {
          return {
            ok: true,
            value: { site: displayHost(c.frame.url) ?? "", title: c.title, card: { brand: c.brand, last4: c.card.number.slice(-4), expiry: displayExpiry(c.card.expiry) ?? "" } },
          };
        }
        // …the existing login branch…
      }
      case "save_confirm": {
        const c = liveCardSave(tabId, req.token);
        if (c) {
          const title = req.title?.trim() || c.title;
          try {
            await deps.client.request({
              type: "save_card",
              url: c.frame.url,
              title,
              number: c.card.number,
              expiry: c.card.expiry,
              ...(c.card.verificationNumber ? { verificationNumber: c.card.verificationNumber } : {}),
              ...(c.card.cardholderName ? { cardholderName: c.card.cardholderName } : {}),
            });
          } catch (e) {
            return fail(e);
          } finally {
            dropCardSave(tabId, true);
          }
          return { ok: true, value: null };
        }
        // …the existing login branch…
      }
```

      In `save_dismiss`: `if (liveCardSave(tabId, req.token)) dropCardSave(tabId, true);` before the login line. In `save_resize`: `const s = liveCardSave(tabId, req.token) ?? liveSave(tabId, req.token);`. Both kinds of pending save have `frame` and `token`, so the rest of the branch is unchanged.
  - **`reset`:** add `for (const tabId of [...cardSaves.keys()]) dropCardSave(tabId, true); scans.clear();`. **`forgetTab`:** add `dropCardSave(tabId, false);`. **Return:** `{ handleContent, handleInline, pickFill, fillCard, scanCards, reset, forgetTab }`.

- [ ] **Step 5: Wire `background/index.ts`**
  - Widen `sendToTab`'s parameter type to `BgWaResult | BackgroundToContent`.
  - Pass it to the inline handler: `const inline = createInlineHandler({ …, sendToTab: (tabId, msg) => sendToTab(tabId, msg) });`.
  - `inline` is declared after `passkeys`. `sendToTab` is a function declaration above both, so ordering is fine.

- [ ] **Step 6: Run the tests**

Run: `cd apps/extension && npx vitest run src/background && npx tsc --noEmit -p .`
Expected: all pass, including the existing `inline-handler.test.ts`. `menu_state` for login and identity menus is unchanged.

- [ ] **Step 7: Commit**

```bash
git add apps/extension/src/background
git commit -m "feat(extension): card menu sessions, multi-frame card fill, hosted menus, card save prompt"
```

---

### Task 9: Content script — card menus, hosting, scan, fill and typed cards

**Files:**
- Modify: `apps/extension/src/content/index.ts`
- Modify: `apps/extension/src/autofill/text.ts` (add `PAY_WORDS`)
- Test: `apps/extension/src/content/content.test.ts`

**Interfaces:**
- Consumes: Tasks 5, 6, 7 and 8 (the message shapes).
- Produces: `menuKindFor(field)` returns `{ kind: "card"; cardRoles: CardRole[] }` for card fields.

- [ ] **Step 1: Write the failing tests** (append to `content/content.test.ts`)

```ts
describe("card fields", () => {
  const set = (html: string) => (document.body.innerHTML = html);
  const f = (sel: string) => document.querySelector(sel) as HTMLInputElement;
  const kindFor = async (el: HTMLInputElement) => (await import("./index")).menuKindFor(el);
  const CHECKOUT = `<form><input id="num" autocomplete="cc-number"><input id="exp" autocomplete="cc-exp" placeholder="MM/AA"><input id="cvv" autocomplete="cc-csc" maxlength="4"><button type="submit">Pagar</button></form>`;

  it("opens the card menu on a card field, with the roles it can fill", async () => {
    set(CHECKOUT);
    expect(await kindFor(f("#num"))).toEqual({ kind: "card", cardRoles: ["number", "expiryMonth", "expiryYear", "verificationNumber"] });
  });

  it("fills a card for a fill on this origin, without a token (another frame of the tab)", () => {
    set(CHECKOUT);
    const reply = deliver({
      type: "bg_fill",
      origin: location.origin,
      token: null,
      fill: { kind: "card", values: [{ role: "number", value: "4111111111111111" }, { role: "expiryMonth", value: "4" }, { role: "expiryYear", value: "2033" }] },
      submit: false,
      totp: false,
    });
    expect(reply).toEqual({ filled: 2, pressing: null });
    expect(f("#exp").value).toBe("04/33");
    expect(document.documentElement.outerHTML).not.toContain("4111");
  });

  it("reports its card fields to a scan", () => {
    set(CHECKOUT);
    sent.length = 0;
    deliver({ type: "bg_card_scan", scan: "c".repeat(32) });
    expect(sent).toContainEqual({ type: "cs_card_fields", scan: "c".repeat(32), roles: ["number", "expiryMonth", "expiryYear", "verificationNumber"] });
  });

  it("hosts a child frame's menu over that frame's iframe", () => {
    set(`<iframe id="stripe"></iframe>`);
    (chrome.runtime as unknown as { getFrameId: (el: Element) => number }).getFrameId = (el) => (el.id === "stripe" ? 7 : -1);
    const reply = deliver({ type: "bg_host_menu", token: "d".repeat(32), frameId: 7, anchor: { top: 0, left: 0, width: 200, height: 20 }, rows: 2 });
    expect(reply).toEqual({ ok: true });
    expect(document.querySelector(`iframe[src$="#${"d".repeat(32)}"]`)).not.toBeNull();
    expect(deliver({ type: "bg_host_menu", token: "e".repeat(32), frameId: 9, anchor: { top: 0, left: 0, width: 200, height: 20 }, rows: 1 })).toEqual({ ok: false });
    deliver({ type: "bg_close_menu", token: "d".repeat(32) });
    expect(document.querySelector(`iframe[src$="#${"d".repeat(32)}"]`)).toBeNull();
  });

  it("offers to save a masked number the user typed, and not one a page script wrote", async () => {
    set(CHECKOUT);
    sent.length = 0;
    const typed = (sel: string, v: string) => {
      f(sel).value = v;
      markUserEdit(f(sel));
    };
    typed("#num", "4000056655665556");
    f("#num").value = "4000 0566 5566 5556"; // the site's mask
    typed("#exp", "01/30");
    document.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true }));
    expect(sent).toContainEqual({
      type: "cs_card_submit",
      card: { number: "4000056655665556", expiry: "2030-01", verificationNumber: null, cardholderName: null },
    });

    set(CHECKOUT);
    sent.length = 0;
    f("#num").value = "4000056655665556"; // page script
    typed("#exp", "01/30");
    await new Promise((r) => setTimeout(r, 1100)); // past the 1 s submit debounce
    document.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true }));
    expect(sent.some((m) => (m as { type: string }).type === "cs_card_submit")).toBe(false);
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/content/content.test.ts -t "card fields"`
Expected: FAIL; `menuKindFor` returns the login or identity kind, or null.

- [ ] **Step 3: Add `PAY_WORDS` to `autofill/text.ts`**

```ts
/** Labels of buttons that pay for a checkout (a typed card is read on these). */
export const PAY_WORDS = [
  "pay",
  "pay now",
  "place order",
  "complete order",
  "complete purchase",
  "buy now",
  "confirm payment",
  "pagar",
  "finalizar",
  "finalizar compra",
  "finalizar pedido",
  "comprar",
  "confirmar pagamento",
  "concluir compra",
];
```

- [ ] **Step 4: Change `content/index.ts`**
  - **Imports:** add `import { cardFieldsForFill, cardGroupFor, findCardGroup } from "../autofill/card";`, `import { cardRolesToFill, fillCard, readCardSubmission } from "../autofill/card-fill";` and `import type { CardRole } from "@havenkeys/protocol";`. Add `isRendered` to the `../autofill/group` import, `PAY_WORDS` to the `../autofill/text` import, and `parseHostReply` is not needed here.
  - **Header comment:** add a bullet: "Cards: a card field gets the card menu; a processor frame's menu is drawn by the top frame over that frame (bg_host_menu); every frame answers a card scan with its roles; a card the user typed is reported on submit, top frame only."
  - **`menuKindFor`:** the return type becomes `{ kind: MenuKind; roles?: IdentityRole[]; cardRoles?: CardRole[] } | null`. Its first lines, after `const env = defaultEnv();`:

```ts
  const card = cardGroupFor(field, env);
  if (card) {
    const cardRoles = cardRolesToFill(card, env);
    return cardRoles.length > 0 ? { kind: "card", cardRoles } : null;
  }
```

  - **`OpenMenu`:** becomes `{ frame: InlineFrame | null; token: string; field: HTMLInputElement; rows: number; height: number | null }`. `frame` is null when the top frame hosts the menu.
  - **`closeMenu`:**

```ts
  function closeMenu(tellBackground: boolean): void {
    if (!menu) return;
    const { frame, field, token } = menu;
    menu = null;
    frame?.remove();
    picked = { token, field, until: Date.now() + PICK_WINDOW_MS };
    if (tellBackground) void send({ type: "cs_close_menu", token });
    if (icon && deepActiveElement() !== icon.field) hideIcon();
  }
```

  - **`placeMenu`:** after the fillable check, `if (!menu.frame) return;`, then `menu.frame.place(…)`.
  - **`maybeOpen`:**
    - Add to the request: `...(choice.cardRoles ? { cardRoles: choice.cardRoles } : {})` and `...(kind === "card" && window.top !== window ? { anchor: anchorOf(field) } : {})`.
    - Read `hosted` from the reply.
    - Replace the frame creation:

```ts
    closeMenu(true);
    const token = reply.token;
    if (reply.hosted === true) {
      menu = { frame: null, token, field, rows, height: null };
      return;
    }
    const frame = new InlineFrame("menu.html", token, menuBox(field.getBoundingClientRect(), rows, viewport()), () => {
      if (menu?.token === token) closeMenu(true);
    });
    menu = { frame, token, field, rows, height: null };
```

    - Widen the reply cast to `{ ok?: unknown; token?: unknown; rows?: unknown; hosted?: unknown }`.
    - Add the helper near `viewport()`:

```ts
  function anchorOf(field: HTMLInputElement) {
    const r = field.getBoundingClientRect();
    return { top: Math.round(r.top), left: Math.round(r.left), width: Math.round(r.width), height: Math.round(r.height) };
  }
```

  - **Menu listeners:** `focusin` uses `if (menu?.frame && e.composedPath()[0] === menu.frame.el) return;`. `ArrowDown` uses `menu.frame?.focus()`. The `bg_close_menu` / `bg_resize_menu` checks use `menu?.token === m.token`; resize also requires `menu.frame`.
  - **Hosting (top frame):** add after the save prompt helpers:

```ts
  // ------------------------------------------------------------ hosted card menus

  /** Iframes examined when hosting a child frame's menu. */
  const MAX_HOST_IFRAMES = 50;
  /** A menu the top frame shows for a child frame (a processor's small card iframe). */
  let hosted: { frame: InlineFrame; anchor: DOMRect; rows: number; height: number | null } | null = null;

  function closeHosted(tell: boolean): void {
    if (!hosted) return;
    const token = hosted.frame.token;
    hosted.frame.remove();
    hosted = null;
    if (tell) void send({ type: "cs_close_menu", token });
  }

  function rect(left: number, top: number, width: number, height: number): DOMRect {
    return { left, top, width, height, right: left + width, bottom: top + height, x: left, y: top, toJSON: () => ({}) } as DOMRect;
  }

  /** Show menu `m.token` over the iframe of child frame `m.frameId`. False when it is not a direct child. */
  function hostMenu(m: Extract<BackgroundToContent, { type: "bg_host_menu" }>): boolean {
    if (window.top !== window) return false;
    const getFrameId = (chrome.runtime as { getFrameId?: (target: Element) => number }).getFrameId;
    if (typeof getFrameId !== "function") return false;
    const el = Array.from(document.querySelectorAll("iframe"))
      .slice(0, MAX_HOST_IFRAMES)
      .find((f) => {
        try {
          return getFrameId(f) === m.frameId;
        } catch {
          return false;
        }
      });
    if (!el || !isRendered(el)) return false;
    const r = el.getBoundingClientRect();
    const anchor = rect(r.left + el.clientLeft + m.anchor.left, r.top + el.clientTop + m.anchor.top, m.anchor.width, m.anchor.height);
    closeHosted(true);
    const token = m.token;
    hosted = {
      frame: new InlineFrame("menu.html", token, menuBox(anchor, m.rows, viewport()), () => {
        if (hosted?.frame.token === token) closeHosted(true);
      }),
      anchor,
      rows: m.rows,
      height: null,
    };
    return true;
  }
```

  - **Page events and hosting:**
    - In `reposition`, close a hosted menu: its anchor is stale after a scroll. Add `if (hosted) closeHosted(true);` at the top of `reposition`, and include `hosted` in its early-return condition.
    - In the `pointerdown` handler, after the `isTrusted` check: `if (hosted && e.composedPath()[0] !== hosted.frame.el) closeHosted(true);`.
    - In `pagehide`: `closeHosted(true);`.
  - **Fill:** in `handleFill`, after the identity branch:

```ts
    if (m.fill.kind === "card") {
      let target: typeof picked = null;
      if (m.token !== null) {
        target = picked && picked.token === m.token && picked.until > Date.now() ? picked : null;
        picked = null;
        if (!target || !target.field.isConnected) return none;
      }
      const g = target ? cardGroupFor(target.field, env) : cardFieldsForFill(document, env);
      return { filled: g ? fillCard(g, m.fill.values, env) : 0, pressing: null };
    }
```

  - **Background messages:** in the `onMessage` switch, replace the temporary Task 7 cases:

```ts
      case "bg_card_scan": {
        const env = defaultEnv();
        const g = cardFieldsForFill(document, env);
        const roles = g ? cardRolesToFill(g, env) : [];
        if (roles.length > 0) void send({ type: "cs_card_fields", scan: m.scan, roles });
        return false;
      }
      case "bg_host_menu":
        sendResponse({ ok: hostMenu(m) });
        return false;
```

    `bg_close_menu` also closes a hosted menu: `if (hosted?.frame.token === m.token) closeHosted(false);`. `bg_resize_menu` also resizes it: `if (hosted?.frame.token === m.token) { hosted.height = m.height; hosted.frame.place(menuBox(hosted.anchor, hosted.rows, viewport(), m.height)); }`.
  - **Typed cards:** add after `captureFrom`:

```ts
  let lastCardSubmit = 0;

  /** A card the user typed in `root`, offered for saving. Top frame only (spec §5.6). */
  function captureCard(root: ParentNode): void {
    if (window.top !== window) return;
    const now = Date.now();
    if (now - lastCardSubmit < SUBMIT_DEBOUNCE_MS) return;
    const g = findCardGroup(root, defaultEnv());
    const card = g ? readCardSubmission(g) : null;
    if (!card) return;
    lastCardSubmit = now;
    void send({ type: "cs_card_submit", card });
  }

  /** The card group a pay button belongs to: its form, or the nearest container holding card fields. */
  function cardRootForButton(button: Element): ParentNode | null {
    const form = button.closest("form");
    if (form) return form;
    let node: Element | null = button.parentElement;
    for (let i = 0; node && i < 6; i++, node = node.parentElement) {
      if (findCardGroup(node, defaultEnv())) return node;
    }
    return null;
  }
```

    Call it from:
    - the `submit` listener: `if (form instanceof HTMLFormElement) { captureFrom(form); captureCard(form); }`;
    - the `Enter` key branch: `if (input?.form) captureCard(input.form);`;
    - the `click` listener, before the login check:

```ts
      const button = (target as Element | undefined)?.closest?.('button, input[type="submit"], input[type="image"], [role="button"]');
      if (button) {
        const label = normalize(`${button.textContent ?? ""} ${button.getAttribute("aria-label") ?? ""}`);
        if (isSubmitLike(button) || hasAny(label, PAY_WORDS)) {
          const root = cardRootForButton(button);
          if (root) captureCard(root);
        }
      }
```

    The existing `el`/`isSubmitLike` login code stays after it.

- [ ] **Step 5: Run the tests**

Run: `cd apps/extension && npx vitest run src/content src/autofill && npx tsc --noEmit -p .`
Expected: all pass. The existing login, identity and SSO content tests are unchanged.

- [ ] **Step 6: Commit**

```bash
git add apps/extension/src/content/index.ts apps/extension/src/content/content.test.ts apps/extension/src/autofill/text.ts
git commit -m "feat(extension): card menus in the content script, top-frame hosting, scan, fill and typed cards"
```

---

### Task 10: Menu and save pages — card rows, brand logos, the card save prompt

**Files:**
- Modify: `apps/extension/src/menu/icons.ts`
- Modify: `apps/extension/src/menu/menu.ts`
- Modify: `apps/extension/src/menu/save.ts`
- Modify: `apps/extension/src/menu/inline.css`
- Test: `apps/extension/src/menu/menu.test.ts`, `apps/extension/src/menu/save.test.ts`

**Interfaces:**
- Consumes: Task 7 (`CardRowView`, `CardSaveView`, the `cards` MenuView and the strings) and `@havenkeys/ui/card-brand-icons`.
- Produces: `cardBrandIcon(brand: CardBrandId | null, size?: number): SVGSVGElement`.

- [ ] **Step 1: Write the failing tests**

Append to `menu/menu.test.ts`:

```ts
describe("card rows", () => {
  const trusted = { isTrusted: true } as MouseEvent;
  const view = (cards: object[], insecure = false) => ({ ok: true, value: { state: "cards", site: "shop.com", cards, insecure } });
  const visa = { id: ITEM, title: "Visa", brand: "visa", last4: "1111", expiry: "04/33", expired: false };

  async function setup(v: object) {
    const handlers = captureClicks();
    replies = [v];
    await load();
    (globalThis as unknown as { chrome: { runtime: { sendMessage: unknown } } }).chrome.runtime.sendMessage = async (m: unknown) => {
      asked.push(m);
      return { ok: true, value: null };
    };
    await vi.advanceTimersByTimeAsync(1000);
    return handlers;
  }

  it("lists cards with their logo, last four and expiry, and names the page", async () => {
    const handlers = await setup(view([visa, { ...visa, id: "11111111-2222-4333-8444-555555555555", title: "Old", expiry: "01/20", expired: true }]));
    expect(document.getElementById("site")!.textContent).toBe("Fill on shop.com");
    const rows = Array.from(document.querySelectorAll<HTMLButtonElement>("button.row"));
    expect(rows[0]!.textContent).toContain("•••• 1111 · 04/33");
    expect(rows[0]!.querySelector("svg")).not.toBeNull();
    expect(rows[1]!.classList.contains("expired")).toBe(true);
    expect(rows[1]!.textContent).toContain("Expired");
    handlers.get(rows[0]!)?.(trusted);
    await vi.advanceTimersByTimeAsync(0);
    expect(asked.at(-1)).toEqual({ type: "menu_pick_card", token: TOKEN, itemId: ITEM });
  });

  it("explains http pages without buttons", async () => {
    await setup(view([], true));
    expect(document.body.textContent).toContain("HavenKeys fills cards only on secure (https) pages.");
    expect(document.querySelector("button.row")).toBeNull();
  });

  it("says when no card is saved, without buttons", async () => {
    await setup(view([]));
    expect(document.body.textContent).toContain("No cards saved");
    expect(document.querySelector("button.row")).toBeNull();
  });
});
```

Append to `menu/save.test.ts`:

```ts
describe("card save prompt", () => {
  it("shows the card's logo, last four and expiry with an editable name", async () => {
    await openPrompt({ site: "shop.com", title: "Visa", card: { brand: "visa", last4: "5556", expiry: "01/30" } });
    expect(document.getElementById("question")!.textContent).toBe("Save this card to HavenKeys?");
    expect(document.getElementById("detail")!.textContent).toContain("•••• 5556 · 01/30");
    expect(document.querySelector("#detail svg")).not.toBeNull();
    expect(titleField().hidden).toBe(false);
    expect(titleInput().value).toBe("Visa");
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/menu/menu.test.ts src/menu/save.test.ts`
Expected: FAIL; no rows are rendered for `state: "cards"`.

- [ ] **Step 3: Implement `cardBrandIcon` in `menu/icons.ts`**

Refactor `providerIcon`'s body into `function drawIcon(icon: ProviderIcon, size: number, cls: string): SVGSVGElement` (identical code, the class passed in). Then:

```ts
import { CARD_BRAND_ICONS, GENERIC_CARD_ICON } from "@havenkeys/ui/card-brand-icons";
import type { ProviderIcon } from "@havenkeys/ui/provider-icons";
import type { CardBrandId } from "@havenkeys/protocol";

export function providerIcon(p: SsoProvider, size = 18): SVGSVGElement {
  return drawIcon(PROVIDER_ICONS[p], size, "provider-icon");
}

/** A card network's logo; the generic card for "other" and unknown brands. */
export function cardBrandIcon(brand: CardBrandId | null, size = 24): SVGSVGElement {
  const icon = brand && brand !== "other" && brand in CARD_BRAND_ICONS ? CARD_BRAND_ICONS[brand as keyof typeof CARD_BRAND_ICONS] : GENERIC_CARD_ICON;
  return drawIcon(icon, size, "card-brand-icon");
}
```

If `packages/ui/package.json` has no `"./card-brand-icons"` export, add `"./card-brand-icons": "./src/card-brand-icons.ts"` (the card-item plan should have added it).

- [ ] **Step 4: Render the cards view in `menu.ts`**

Add `cardBrandIcon` to the `./icons` import and `CardRowView` to the `../messaging/inline` import, then:

```ts
function cardRow(t: string, c: CardRowView): HTMLButtonElement {
  const detail = [msg.menu.cardRow(c.last4, c.expiry), c.expired ? msg.menu.cardExpired : ""].filter(Boolean).join(" · ");
  const b = row(cardBrandIcon(c.brand), c.title || msg.menu.cardFallback, detail, () => pick({ type: "menu_pick_card", token: t, itemId: c.id }), {
    title: c.title === "",
    detail: true,
  });
  if (c.expired) b.classList.add("expired");
  return b;
}
```

In `render`, before `site.textContent = view.site;`:

```ts
  if (view.state === "cards") {
    // The page the card goes to, named once in the header (spec §5.2).
    site.textContent = msg.menu.cardFillOn(view.site);
    if (view.insecure) main.replaceChildren(message(msg.menu.cardsInsecureTitle, msg.menu.cardsInsecureBody));
    else if (view.cards.length === 0) main.replaceChildren(hintNote(msg.menu.noCardsTitle, msg.menu.noCardsBody));
    else main.replaceChildren(...view.cards.map((c) => cardRow(t, c)));
    return;
  }
```

In `inline.css`, add after the existing `.row` rules:

```css
.row.expired { opacity: 0.6; }
.avatar-icon .card-brand-icon { width: 24px; height: 16px; }
#detail .card-brand-icon { width: 20px; height: 14px; vertical-align: -2px; margin-right: 6px; }
```

- [ ] **Step 5: Render the card prompt in `save.ts`**

Add `cardBrandIcon` from `./icons` and `CardSaveView` to the `../messaging/inline` import, then:

```ts
function showCard(view: CardSaveView): void {
  site.textContent = view.site;
  question.textContent = t.save.cardQuestion;
  confirmBtn.textContent = t.save.save;
  titleField.hidden = false;
  titleInput.value = view.title;
  detail.classList.add("copy");
  delete detail.dataset.truncate;
  detail.replaceChildren(cardBrandIcon(view.card.brand, 20), document.createTextNode(t.menu.cardRow(view.card.last4, view.card.expiry)));
  confirmBtn.disabled = false;
}
```

In `init`: `const r = await ask<SaveView | CardSaveView>({ type: "save_state", token }); if (!r.ok) return fail(r.message); if ("card" in r.value) showCard(r.value); else show(r.value);`.

- [ ] **Step 6: Run the tests**

Run: `cd apps/extension && npx vitest run src/menu && npx tsc --noEmit -p .`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add apps/extension/src/menu packages/ui/package.json
git commit -m "feat(extension): card rows with brand logos in the menu; the card save prompt"
```

---

### Task 11: Popup — Fill card

**Files:**
- Modify: `apps/extension/src/messaging/popup.ts`
- Modify: `apps/extension/src/background/popup-handler.ts`
- Modify: `apps/extension/src/background/index.ts`
- Modify: `apps/extension/src/popup/popup.ts`
- Modify: `apps/extension/src/popup/popup.css`
- Test: `apps/extension/src/background/popup-handler.test.ts`

**Interfaces:**
- Consumes: Task 8 (`inline.fillCard`, `inline.scanCards`, `cardRows`) and Task 10 (`cardBrandIcon`).
- Produces:
  - The popup request `{ type: "popup_fill_card"; itemId: string }`.
  - `PopupState` unlocked gains `cards?: CardRowView[]`.
  - `createPopupHandler`'s 6th parameter: `cards?: { scan(tabId: number): Promise<boolean>; fill(tabId: number, topUrl: string, itemId: string): Promise<number> }`.

- [ ] **Step 1: Write the failing tests** (append to `popup-handler.test.ts`, following its existing `setup`/fake-client pattern)

```ts
describe("popup cards", () => {
  const VISA = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
  const find = { type: "find_cards", insecure: false, cards: [{ id: VISA, title: "Visa", brand: "visa", last4: "1111", expiry: "2033-04" }] };
  function handler(url: string, scan: boolean, filled = 2) {
    const requests: Request[] = [];
    const fills: Array<[number, string, string]> = [];
    const client = {
      request: async (r: Request) => {
        requests.push(r);
        switch (r.type) {
          case "status":
            return { type: "status", state: "unlocked", vaultExists: true };
          case "find_matches":
            return { type: "find_matches", matches: [] };
          case "find_identity":
            throw new BridgeError("not_found", "x");
          case "find_cards":
            return find;
          default:
            throw new Error(r.type);
        }
      },
    };
    const h = createPopupHandler(client as never, async () => ({ id: 9, url }), async () => 0, undefined, undefined, {
      scan: async () => scan,
      fill: async (tabId, topUrl, itemId) => {
        fills.push([tabId, topUrl, itemId]);
        return filled;
      },
    });
    return { h, requests, fills };
  }

  it("lists cards only on https pages with card fields", async () => {
    expect(await handler("https://shop.com/checkout", true).h.handle({ type: "popup_state" })).toMatchObject({
      value: { kind: "unlocked", cards: [{ id: VISA, last4: "1111", expiry: "04/33", expired: false }] },
    });
    expect((await handler("https://shop.com/", false).h.handle({ type: "popup_state" })).ok).toBe(true);
    expect(await handler("https://shop.com/", false).h.handle({ type: "popup_state" })).not.toMatchObject({ value: { cards: expect.anything() } });
    expect(await handler("http://shop.com/", true).h.handle({ type: "popup_state" })).not.toMatchObject({ value: { cards: expect.anything() } });
  });

  it("fills an offered card into the tab, and reports no card form", async () => {
    const { h, fills } = handler("https://shop.com/checkout?x=1", true);
    expect(await h.handle({ type: "popup_fill_card", itemId: VISA })).toEqual({ ok: true, value: null });
    expect(fills).toEqual([[9, "https://shop.com/checkout", VISA]]);
    expect(await handler("https://shop.com/", true, 0).h.handle({ type: "popup_fill_card", itemId: VISA })).toEqual({ ok: false, message: "No card form found on this page." });
    expect(await h.handle({ type: "popup_fill_card", itemId: "11111111-2222-4333-8444-555555555555" })).toMatchObject({ ok: false });
  });
});
```

Import `BridgeError` and `Request` at the top if the file does not already.

- [ ] **Step 2: Run to see it fail**

Run: `cd apps/extension && npx vitest run src/background/popup-handler.test.ts -t "popup cards"`
Expected: FAIL; `popup_fill_card` is an unknown request type.

- [ ] **Step 3: Implement**
  - **`messaging/popup.ts`:**
    - Add `| { type: "popup_fill_card"; itemId: string }` to `PopupRequest`, and parse it with the `popup_fill` group: `case "popup_fill_card":` joins the list of `itemId` requests.
    - `PopupState` unlocked: `{ kind: "unlocked"; site: string | null; matches: Match[]; identity: { title: string } | null; cards?: CardRowView[] }`, importing `CardRowView` from `./inline`.
  - **`popup-handler.ts`:**
    - Add the 6th parameter `cards?: CardAccess` with `export interface CardAccess { scan(tabId: number): Promise<boolean>; fill(tabId: number, topUrl: string, itemId: string): Promise<number> }`, and import `cardRows` from `./card-rows`.
    - In `state()`, read the tab once (`const tab = await activeTab(); const url = pageUrlForRequest(tab?.url);`). After the identity lookup:

```ts
      let cardList: CardRowView[] | undefined;
      if (cards && tab && url.startsWith("https:")) {
        try {
          const found = await client.request({ type: "find_cards", url });
          if (found.cards.length > 0 && (await cards.scan(tab.id))) cardList = cardRows(found.cards, Date.now());
        } catch {
          cardList = undefined;
        }
      }
      return { kind: "unlocked", site: displayHost(url), matches: found.matches, identity, ...(cardList ? { cards: cardList } : {}) };
```

    - The `found` of `find_matches` keeps its name; call the cards result `foundCards` to avoid shadowing.
    - Add the handler case:

```ts
      case "popup_fill_card": {
        // Tab and URL are read here, never taken from the popup; Rust checks every frame.
        const tab = await activeTab();
        const url = pageUrlForRequest(tab?.url);
        if (!tab || !url || !cards) return { ok: false, message: t.errors.pageNotSupported };
        try {
          const offered = await client.request({ type: "find_cards", url });
          if (!offered.cards.some((c) => c.id === req.itemId)) return { ok: false, message: t.errors.unknownItem };
          const filled = await cards.fill(tab.id, url, req.itemId);
          return filled > 0 ? { ok: true, value: null } : { ok: false, message: t.errors.noCardForm };
        } catch (e) {
          return fail(e);
        }
      }
```

  - **`background/index.ts`:**

```ts
/** Popup cards: the top frame gets the content script (the others have it where site access is granted). */
async function injectTop(tabId: number): Promise<boolean> {
  try {
    await chrome.scripting.executeScript({ target: { tabId, frameIds: [0] }, files: ["content.js"] });
    return true;
  } catch {
    return false;
  }
}

const popup = createPopupHandler(client, activeTab, fillTab, ssoTab, scanIdentity, {
  scan: async (tabId) => (await injectTop(tabId)) && (await inline.scanCards(tabId)).length > 0,
  fill: async (tabId, topUrl, itemId) => ((await injectTop(tabId)) ? inline.fillCard(tabId, topUrl, null, itemId) : 0),
});
```

    Replace the existing `const popup = createPopupHandler(…)` line with this block.
  - **`popup/popup.ts`:** import `cardBrandIcon` from `../menu/icons` and `CardRowView` from `../messaging/inline`, then add:

```ts
function cardRow(c: CardRowView): HTMLElement {
  const status = h("div", { className: "row-status" });
  const fill = smallButton(t.popup.fill, t.popup.fillCardTitle);
  fill.addEventListener("click", () => void fillFromPopup(fill, { type: "popup_fill_card", itemId: c.id }, status));
  const detail = [t.menu.cardRow(c.last4, c.expiry), c.expired ? t.menu.cardExpired : ""].filter(Boolean).join(" · ");
  return h(
    "li",
    { className: c.expired ? "item card expired" : "item card" },
    h("span", { className: "avatar card-avatar" }, cardBrandIcon(c.brand, 24)),
    h("div", { className: "who" }, truncates(h("div", { className: "title", text: c.title || t.menu.cardFallback })), h("div", { className: "user copy", text: detail })),
    h("div", { className: "actions" }, fill),
    status,
  );
}
```

    In `render` (unlocked), after the identity row: `if (state.cards && state.cards.length > 0) parts.push(h("div", { className: "section-title", text: t.popup.cardsTitle }), h("ul", { className: "list" }, ...state.cards.map(cardRow)));`.
  - **`popup/popup.css`:**

```css
.section-title { margin: 12px 16px 4px; font-size: 12px; font-weight: 600; color: var(--hk-text-muted, inherit); text-transform: uppercase; letter-spacing: 0.04em; }
.item.card.expired { opacity: 0.6; }
.card-avatar { display: inline-flex; align-items: center; justify-content: center; }
```

    First check which variable name the popup's muted text uses (`grep -n "muted" apps/extension/src/popup/popup.css`) and use that name.

- [ ] **Step 4: Run the tests**

Run: `cd apps/extension && npx vitest run src/background src/popup && npx tsc --noEmit -p .`
Expected: all pass. Existing popup states without `cards` are unchanged.

- [ ] **Step 5: Commit**

```bash
git add apps/extension/src/messaging/popup.ts apps/extension/src/background apps/extension/src/popup
git commit -m "feat(extension): Fill card from the toolbar popup"
```

---

### Task 12: Manifest, docs, UI check, full verification

**Files:**
- Modify: `apps/extension/manifest/firefox.json`, `docs/autofill.md`, `docs/native-messaging.md`, `docs/security-model.md`, `CLAUDE.md`, `README.md`, `tools/ui-check/extension.mjs`, `docs/superpowers/specs/2026-09-29-card-autofill-design.md` (Status → accepted)

- [ ] **Step 1: Firefox data collection.** The extension now handles payment data. In `manifest/firefox.json`, change `"required": ["authenticationInfo", "personallyIdentifyingInfo"]` to `"required": ["authenticationInfo", "personallyIdentifyingInfo", "financialAndPaymentInfo"]`. Then run `cd apps/extension && npx vitest run src/i18n/manifest.test.ts` and update that test if it pins the list.

- [ ] **Step 2: Docs**
  - `docs/native-messaging.md`: add `find_cards`, `fill_card` and `save_card` to the request table, with their fields, result, rate class and what Rust checks (spec §6 plus this plan's Global Constraints). Add the processor list, with its sources, to §5 "Authorization".
  - `docs/autofill.md`: add a new "Cards" section. It covers:
    - the signals (autocomplete table, words, the weak month/year/code words and when they count, split numbers);
    - qualifying groups (including a processor frame's lone `cc-*` field);
    - the frames of one pick (scan, eligibility lookup, one `fill_card`);
    - the hosted menu;
    - the writing rules (expiry formats, month and brand selects, number grouping, CVV never cut, empty-only);
    - the typed-card reader (user-typed digits only, top frame).

    At lines ~176 and ~1041, change "Card fields … refused" to say that the login and identity classifiers refuse them and that the card classifier owns them.
  - `docs/security-model.md`: add the not-site-bound card decision and its seven rules (spec §2), the processor list and why each entry is there, the three requests in the IPC section, and the Firefox `financialAndPaymentInfo` declaration. Replace the sentence at lines ~821–823 ("Card fields … are refused by the classifier and no card data exists in the identity.") with "Card fields are refused by the identity classifier; cards are filled only through the card flow below, and no card data exists in the identity."
  - `CLAUDE.md` §25: append this note after the identity-autofill note, in the same style:

```markdown
> Amended on 2026-09-29 by
> `docs/superpowers/specs/2026-09-29-card-autofill-design.md`: after the
> user picks a card in the menu or popup, HavenKeys fills that checkout's
> card fields on any https site (not bound to a saved website), including
> the card frames of a fixed list of payment processors in Rust. Nothing is
> filled on http pages, nothing is overwritten or submitted, and a card
> typed into a checkout is saved only after the user confirms.
```

  - `README.md`: add one bullet under "What the browser extension does": filling checkouts from saved cards (https only, processor frames included), and saving a typed card after confirming.
  - Spec status line → `Status: accepted, 2026-09-29.`

- [ ] **Step 3: UI check scenarios.** In `tools/ui-check/extension.mjs`, follow the existing menu, save and popup scenarios and add:
  - `menu-cards-${width}`: `menu_state: ok({ state: "cards", site: "shop.example.com", insecure: false, cards: [visa, elo, expired] })`, where each card is `{ id, title, brand, last4, expiry, expired }`;
  - `menu-cards-insecure-${width}` (`insecure: true, cards: []`);
  - `menu-no-cards-${width}` (`cards: []`);
  - `save-card`: `save_state: ok({ site: "shop.example.com", title: "Mastercard", card: { brand: "mastercard", last4: "7609", expiry: "11/33" } })`;
  - `popup-cards`: a `popup_state` with `cards: [...]` and two cards.

Run: `pnpm ui:check --app=extension`
Expected: `no clipped text, no horizontal overflow.` Look at the card screenshots in `ui-check-output/extension/…`, in both languages and both themes: the logos render and the expired row is dimmed.

- [ ] **Step 4: Full verification**

```bash
cargo fmt --all --check
pnpm lint:rust
cargo test --workspace --exclude havenkeys-server --exclude havenkeys-sync-client
scripts/test-server.sh
(cd packages/protocol && npx vitest run && npx tsc --noEmit -p .)
(cd packages/ui && npx vitest run && npx tsc --noEmit -p .)
(cd apps/extension && npx vitest run && npx tsc --noEmit -p .)
(cd apps/desktop && npx vitest run && npx tsc --noEmit -p .)
cargo audit
pnpm build:extension
```

Expected: everything passes, and `cargo audit` shows only the pre-existing allowed warnings. No new crate or npm dependency was added.

- [ ] **Step 5: Secret-leak review**
  - `grep -rn "console\." apps/extension/src/autofill/card*.ts apps/extension/src/background/card-rows.ts` returns nothing. `hygiene.test.ts` also enforces this.
  - No card value is written to an attribute: the fill tests assert that `innerHTML`/`outerHTML` hold no number.
  - The `Debug` of `CardOffer`, `CardValue`, `CardMatch` and `Request::SaveCard` shows no values (tests in Tasks 1–2).
  - A pending card save is cleared on lock, tab close and TTL (Task 8 tests).

- [ ] **Step 6: Commit**

```bash
git add apps/extension/manifest docs CLAUDE.md README.md tools/ui-check
git commit -m "docs: card autofill: native messaging, autofill rules, security model, CLAUDE.md note"
```

Then report to the user and ask whether to merge `card-autofill` into `main` and push. Previous features were merged with `git merge --no-ff` and pushed on request.

---

## Spec corrections this plan makes (for the reviewer)

1. **Value limit.** Spec §6.1 says `value ≤ 512 bytes`, but a 256-character cardholder name with accents can exceed that. The plan uses 1024 (`MAX_CARD_VALUE_BYTES = 4 * 256`), matching the title limit.
2. **Stripe's subdomains.** Stripe documents `*.js.stripe.com` frames as well as `js.stripe.com`, so the list has one "subdomains of" entry (`PAYMENT_FRAME_PARENTS`). Its check matches on a whole label, so `js.stripe.com.evil.com` and `evil-js.stripe.com` stay denied.
3. **Processor list.**
   - Kept, each confirmed from the processor's own docs or official SDK source:
     - Stripe (`js.stripe.com`, `*.js.stripe.com`);
     - Adyen (the live `checkoutshopper-*.adyen.com` origins: EU, US, AU, APSE, IN, NEA);
     - Braintree (`assets.braintreegateway.com`);
     - Mercado Pago (`secure-fields.mercadopago.com`, `api-static.mercadopago.com`).
   - Left out:
     - Pagar.me (tokenizecard.js), PagSeguro (`PagSeguro.encryptCard`) and Cielo (Silent Order Post): their documented integrations keep the card inputs on the merchant's own page, which the same-site rule already covers. No card-field iframe origin to add.
4. **Menu in a small processor iframe.** The spec does not cover this. A menu drawn inside Stripe's 40-px iframe would be clipped. The plan adds `bg_host_menu`: the top frame draws the menu over that iframe, found with `runtime.getFrameId` where it exists (Firefox only; Chromium has no `runtime.getFrameId`), otherwise by the one iframe whose `src` without query and fragment equals the frame's URL (`content/host-frame.ts`, ruling A4). It falls back to the frame itself when the iframe is nested deeper or cannot be told apart.
5. **Frames Rust would deny.** The spec says one bad frame denies the whole `fill_card`. So the background checks each extra frame with a `find_cards` lookup first and drops those Rust denies (an ad iframe with card-like inputs). Otherwise one unrelated frame would make every card fill fail on that page.
6. **"No cards saved" row.** The spec says the row opens the desktop app, but no request opens the app without an item (`open_item` needs a login saved for the page). The row is informational, like the identity's "missing" row.
7. **Save prompt: excluded sites.** Spec §5.6's condition "the site is not excluded from save prompts" refers to a feature that does not exist. It is dropped.
8. **Save prompt: navigation.** The spec says the pending card is dropped on navigation. Checkouts navigate on submit, so it would never be seen. It survives navigation for its 2-minute TTL, exactly as login save prompts do.
9. **Save prompt: which frames.** It is offered only for the **top frame**, not same-site frames. The background cannot tell same-site without the public-suffix list, and a prompt that Rust then refuses would be worse.
10. **"Fill on *site*".** It is shown once, in the menu header, instead of on every row: the rows stay short and the header is where the site already appears.
11. **Popup.** The popup lists the cards, each with its own Fill button, instead of a single "Fill card" button followed by a picker. The effect is the same with one click less.
12. **Masked number fields.** Sites often reformat the number as it is typed. The spec's "a number the user typed" is implemented as "the digits the user typed equal the field's digits" (`typedByUser`), so the save prompt still appears on masked fields.
