//! Cards in the desktop app (spec 2026-09-29-card-item §5.1).
//!
//! The core owns the item and its rules. The renderer gets a card's values
//! without its number and verification number when a card opens; those two
//! come one at a time, on an explicit reveal. Copies go from Rust to the
//! clipboard.

use crate::commands::{copy_from_vault, CopyResult};
use crate::state::{AppState, CmdResult};
use havenkeys_core::card::{self, CardBrand, CardExpiry, CardField};
use havenkeys_core::SecretString;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

/// An opened card, without the two values that need a reveal.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardView {
    cardholder_name: Option<SecretString>,
    /// The user's choice; `None` means detected from the number.
    brand: Option<CardBrand>,
    expiry: Option<CardExpiry>,
    notes: Option<SecretString>,
    has_number: bool,
    has_verification_number: bool,
}

/// The two values shown only after the eye is clicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RevealField {
    Number,
    VerificationNumber,
}

impl From<RevealField> for CardField {
    fn from(f: RevealField) -> Self {
        match f {
            RevealField::Number => CardField::Number,
            RevealField::VerificationNumber => CardField::VerificationNumber,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardNumberCheck {
    brand: Option<CardBrand>,
    check_digit_ok: bool,
}

/// Brand and check digit of a number being typed. Spaces and `-` are
/// ignored; anything else that is not a digit gives no brand.
fn check(number: &str) -> CardNumberCheck {
    let digits = SecretString::new(number.chars().filter(|c| *c != ' ' && *c != '-').collect());
    let d = digits.expose();
    if d.is_empty() || !d.bytes().all(|b| b.is_ascii_digit()) {
        return CardNumberCheck {
            brand: None,
            check_digit_ok: false,
        };
    }
    CardNumberCheck {
        brand: card::detect_brand(d),
        check_digit_ok: (card::MIN_NUMBER_DIGITS..=card::MAX_NUMBER_DIGITS).contains(&d.len())
            && card::check_digit_ok(d),
    }
}

#[tauri::command]
pub fn reveal_card(state: State<'_, AppState>, id: Uuid) -> CmdResult<CardView> {
    state.touch();
    let f = state.vault()?.card_fields(id)?;
    let has_number = f.number.is_some();
    let has_verification_number = f.verification_number.is_some();
    Ok(CardView {
        cardholder_name: f.cardholder_name,
        brand: f.brand,
        expiry: f.expiry,
        notes: f.notes,
        has_number,
        has_verification_number,
    })
}

#[tauri::command]
pub fn reveal_card_field(
    state: State<'_, AppState>,
    id: Uuid,
    field: RevealField,
) -> CmdResult<SecretString> {
    state.touch();
    Ok(state.vault()?.card_value(&id, field.into())?)
}

/// Copy one card value. Read here and handed to the clipboard, cleared
/// after the vault's delay; it does not pass through the renderer.
#[tauri::command]
pub fn copy_card_field(
    state: State<'_, AppState>,
    id: Uuid,
    field: CardField,
) -> CmdResult<CopyResult> {
    copy_from_vault(&state, |v| v.card_value(&id, field))
}

/// For the editor's live logo and warning. Nothing is kept.
#[tauri::command]
pub fn check_card_number(number: SecretString) -> CardNumberCheck {
    check(number.expose())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reveal(text: &str) -> Result<RevealField, serde_json::Error> {
        serde_json::from_value(serde_json::Value::String(text.into()))
    }

    fn copy(text: &str) -> Result<CardField, serde_json::Error> {
        serde_json::from_value(serde_json::Value::String(text.into()))
    }

    #[test]
    fn only_the_number_and_code_can_be_revealed() {
        assert_eq!(reveal("number").unwrap(), RevealField::Number);
        assert_eq!(
            reveal("verificationNumber").unwrap(),
            RevealField::VerificationNumber
        );
        for bad in [
            "cardholderName",
            "expiry",
            "Number",
            "verification_number",
            "notes",
            "",
        ] {
            assert!(reveal(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn copy_fields_parse() {
        assert_eq!(copy("cardholderName").unwrap(), CardField::CardholderName);
        assert_eq!(copy("expiry").unwrap(), CardField::Expiry);
        for bad in ["brand", "notes", "pin", "cvv"] {
            assert!(copy(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn number_check() {
        let ok = check("5200 8282-8282 8210");
        assert_eq!(ok.brand, Some(CardBrand::Mastercard));
        assert!(ok.check_digit_ok);
        assert!(!check("5200828282828211").check_digit_ok);
        assert_eq!(check("4").brand, Some(CardBrand::Visa));
        assert!(!check("4").check_digit_ok, "too short to pass");
        assert_eq!(
            check("4111x"),
            CardNumberCheck {
                brand: None,
                check_digit_ok: false
            }
        );
        assert_eq!(
            check(""),
            CardNumberCheck {
                brand: None,
                check_digit_ok: false
            }
        );
    }
}
