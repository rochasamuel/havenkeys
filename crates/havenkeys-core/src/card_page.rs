//! Cards for web pages (spec 2026-09-29-card-autofill §6.2, §6.3).
//!
//! Unlike a login, a card is not bound to a site: any https checkout may
//! ask. What protects it is here and in the extension: https only; a frame
//! only when it is the same site as the tab's page or one of the payment
//! processors' card-field origins below; only the roles asked for; and the
//! user's pick in the extension's own menu.

use crate::card::{
    check_digit_ok, clean_number, detect_brand, CardBrand, CardExpiry, CardFields, CardInput,
};
use crate::error::{Error, Result};
use crate::identity_page::{truncate_bytes, MAX_SUMMARY_TITLE_BYTES};
use crate::model::{clean_title, ItemInput, ItemType, SecretUpdate};
use crate::origin::{host_key, same_site, PageUrl};
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
    // A non-empty label, a dot, then the parent: `b.js.stripe.com`, never
    // `evil-js.stripe.com`.
    PAYMENT_FRAME_PARENTS.iter().any(|parent| {
        host.strip_suffix(parent)
            .and_then(|rest| rest.strip_suffix('.'))
            .is_some_and(|label| !label.is_empty())
    })
}

/// May `frame`, in a tab showing `top`, be served a card?
fn frame_allowed(frame: &PageUrl, top: &PageUrl) -> bool {
    frame.is_https() && (same_site(frame, top) || is_payment_frame(frame))
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
        f.debug_struct("CardOffer")
            .field("id", &self.id)
            .finish_non_exhaustive()
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
fn role_value(f: &CardFields, role: CardRole) -> Option<SecretString> {
    let words: Option<Vec<&str>> = f
        .cardholder_name
        .as_ref()
        .map(|n| n.expose().split_whitespace().collect());
    let v = match role {
        CardRole::CardholderName => words.map(|w| w.join(" ")),
        CardRole::CardholderGivenName => words.and_then(|w| w.first().map(|s| (*s).to_owned())),
        CardRole::CardholderFamilyName => words.filter(|w| w.len() > 1).map(|w| w[1..].join(" ")),
        CardRole::Number => f.number.as_ref().map(|n| n.expose().to_owned()),
        CardRole::VerificationNumber => f
            .verification_number
            .as_ref()
            .map(|n| n.expose().to_owned()),
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
        if !top.is_https() {
            return Ok(CardList {
                insecure: true,
                cards: Vec::new(),
            });
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
                    title: truncate_bytes(o.title.clone(), MAX_SUMMARY_TITLE_BYTES),
                    brand: summary.as_ref().map(|s| s.brand),
                    last4: summary.as_ref().and_then(|s| s.last4.clone()),
                    expiry: summary.and_then(|s| s.expiry),
                }
            })
            .collect();
        Ok(CardList {
            insecure: false,
            cards,
        })
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
        if !top.is_https() {
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
                    .filter_map(|r| role_value(&fields, *r).map(|v| (*r, v)))
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
        if !page.is_https() {
            return Err(Error::Denied);
        }
        if let Some(top) = top_url {
            let top = PageUrl::parse(top).ok_or(Error::Denied)?;
            if !top.is_https() || !same_site(&page, &top) {
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
            None => detect_brand(&digits)
                .map_or("Card", |b| b.display_name())
                .to_owned(),
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
                verification_number: card
                    .verification_number
                    .map_or(SecretUpdate::Keep, SecretUpdate::Set),
                expiry,
                notes: None,
            }),
            sections: None,
        };
        let write = self.stage_create(input, now_ms)?;
        Ok(StagedSave {
            item_id: write.item_id,
            write,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(url: &str) -> PageUrl {
        PageUrl::parse(url).unwrap()
    }

    #[test]
    fn same_site_table() {
        for (a, b, want) in [
            ("https://a.example.com/", "https://b.example.com/", true),
            ("https://example.com/", "http://example.com/", true),
            ("https://alice.github.io/", "https://bob.github.io/", false),
            ("https://github.io/", "https://github.io/", true),
            ("http://192.168.1.1/", "http://192.168.1.1:8080/", true),
            ("http://192.168.1.1/", "http://192.168.1.2/", false),
            (
                "https://example.com/",
                "https://example.com.evil.com/",
                false,
            ),
            ("https://nas.internal/", "https://evil.internal/", false),
        ] {
            assert_eq!(same_site(&page(a), &page(b)), want, "{a} {b}");
        }
    }

    #[test]
    fn payment_frame_table() {
        for (url, want) in [
            ("https://js.stripe.com/", true),
            ("https://b.js.stripe.com/x", true),
            ("https://a.b.js.stripe.com/", true),
            ("https://js.stripe.com./", false),
            ("https://evil-js.stripe.com/", false),
            ("https://xjs.stripe.com/", false),
            ("https://stripe.com/", false),
            ("https://js.stripe.com.evil.com/", false),
            ("http://b.js.stripe.com/", false),
            ("https://b.js.stripe.com:443/", true),
            ("https://b.js.stripe.com:8443/", false),
            ("https://api-static.mercadopago.com/", true),
            ("https://x.api-static.mercadopago.com/", false),
        ] {
            assert_eq!(is_payment_frame(&page(url)), want, "{url}");
        }
    }

    #[test]
    fn https_is_the_scheme_only() {
        assert!(page("https://example.com:8443/").is_https());
        assert!(!page("http://example.com/").is_https());
    }
}
