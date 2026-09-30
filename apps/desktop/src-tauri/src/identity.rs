//! The account's one Identity (spec 2026-09-29-identity-item §6.1).
//!
//! The core owns the item and its rules; this module gives the desktop UI
//! the three things it needs: which item the identity is, its values when
//! the user opens it, and one value on the clipboard.

use crate::commands::{copy_from_vault, CopyResult};
use crate::state::{AppState, CmdResult};
use havenkeys_core::identity::{IdentityField, IdentityFields, MAX_CUSTOM_FIELDS};
use havenkeys_core::SecretString;
use serde::{Deserialize, Deserializer, Serialize};
use tauri::State;
use uuid::Uuid;

/// What to copy: a typed field (`"cpf"`, `"address"`, …) or a custom field by
/// position (`"custom:3"`). Anything else fails deserialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyTarget {
    Field(IdentityField),
    Custom(usize),
}

impl<'de> Deserialize<'de> for CopyTarget {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let text = String::deserialize(deserializer)?;
        if let Some(index) = text.strip_prefix("custom:") {
            // Digits only: `usize::from_str` would also take a leading `+`.
            if index.is_empty() || index.len() > 3 || !index.bytes().all(|b| b.is_ascii_digit()) {
                return Err(D::Error::custom("invalid custom field"));
            }
            let index: usize = index
                .parse()
                .map_err(|_| D::Error::custom("invalid custom field"))?;
            if index >= MAX_CUSTOM_FIELDS {
                return Err(D::Error::custom("invalid custom field"));
            }
            return Ok(CopyTarget::Custom(index));
        }
        serde_json::from_value(serde_json::Value::String(text))
            .map(CopyTarget::Field)
            .map_err(|_| D::Error::custom("unknown identity field"))
    }
}

/// The identity's item ID, for the sidebar entry. Only while unlocked.
#[tauri::command]
pub fn identity_item_id(state: State<'_, AppState>) -> CmdResult<Uuid> {
    Ok(state.vault()?.identity_item_id()?)
}

/// An opened identity: its values and the address block, formatted once,
/// here, so the UI shows exactly what "Copy address" copies.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityView {
    fields: IdentityFields,
    address: Option<SecretString>,
}

/// Every value of an identity, when the user opens it or edits it.
#[tauri::command]
pub fn reveal_identity(state: State<'_, AppState>, id: Uuid) -> CmdResult<IdentityView> {
    state.touch();
    let fields = state.vault()?.reveal_identity(&id)?;
    let address = fields.formatted_address();
    Ok(IdentityView { fields, address })
}

/// Copy one identity value. Read here and handed to the clipboard, cleared
/// after the vault's delay; it does not pass through the renderer.
#[tauri::command]
pub fn copy_identity_field(
    state: State<'_, AppState>,
    id: Uuid,
    field: CopyTarget,
) -> CmdResult<CopyResult> {
    copy_from_vault(&state, |v| match field {
        CopyTarget::Field(f) => v.identity_value(&id, f),
        CopyTarget::Custom(i) => v.identity_custom_value(&id, i),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<CopyTarget, serde_json::Error> {
        serde_json::from_value(serde_json::Value::String(text.into()))
    }

    #[test]
    fn typed_fields_and_the_address_parse() {
        assert_eq!(parse("cpf").unwrap(), CopyTarget::Field(IdentityField::Cpf));
        assert_eq!(
            parse("postalCode").unwrap(),
            CopyTarget::Field(IdentityField::PostalCode)
        );
        assert_eq!(
            parse("address").unwrap(),
            CopyTarget::Field(IdentityField::Address)
        );
    }

    #[test]
    fn custom_fields_parse_by_position_within_the_limit() {
        assert_eq!(parse("custom:0").unwrap(), CopyTarget::Custom(0));
        assert_eq!(parse("custom:49").unwrap(), CopyTarget::Custom(49));
        for bad in [
            "custom:50",
            "custom:",
            "custom:-1",
            "custom:+1",
            "custom:1x",
            "custom:0001",
        ] {
            assert!(parse(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn anything_else_is_refused() {
        for bad in ["", "password", "Cpf", "postal_code", "totp"] {
            assert!(parse(bad).is_err(), "accepted {bad:?}");
        }
        assert!(serde_json::from_str::<CopyTarget>("3").is_err());
    }
}
