//! Bank accounts in the desktop app (spec 2026-11-03-bank-account §6.1).
//!
//! The core owns the item and its rules. The renderer gets an account's
//! values without the account number, its digit and the password when it
//! opens; those come one at a time, on an explicit reveal. Copies go from
//! Rust to the clipboard.

use crate::commands::{copy_from_vault, CopyResult};
use crate::state::{AppState, CmdResult};
use crate::tray::UiLanguage;
use havenkeys_core::bank::{BankAccountFields, BankAccountType};
use havenkeys_core::custom_field::{self, SectionView};
use havenkeys_core::{Error, SecretString};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

/// An opened bank account, without the account, its digit and the password.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankView {
    bank: Option<String>,
    holder_name: Option<SecretString>,
    account_type: Option<BankAccountType>,
    branch: Option<String>,
    branch_digit: Option<String>,
    notes: Option<SecretString>,
    sections: Vec<SectionView>,
    has_account: bool,
    has_password: bool,
}

/// The values shown only after the eye is clicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RevealField {
    Account,
    AccountDigit,
    Password,
}

/// The values that can be copied; the numbers go with their check digit.
/// `Details` is the transfer details, one per line (see `details`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CopyField {
    Bank,
    HolderName,
    Branch,
    Account,
    Password,
    Details,
}

fn join(number: &str, digit: Option<&str>) -> String {
    digit.map_or_else(|| number.to_owned(), |d| format!("{number}-{d}"))
}

fn reveal_value(f: &BankAccountFields, field: RevealField) -> Option<SecretString> {
    match field {
        RevealField::Account => f.account.clone(),
        RevealField::AccountDigit => f.account_digit.clone(),
        RevealField::Password => f.password.clone(),
    }
}

/// What someone needs to send money to this account, in the UI's language:
/// the bank, `Agência 1234-5`, `Conta 12345-6 (corrente)` and `Titular …`,
/// one per line, each only when it has a value.
fn details(f: &BankAccountFields, lang: UiLanguage) -> Option<SecretString> {
    use BankAccountType::*;
    let (branch, account, holder) = match lang {
        UiLanguage::En => ("Branch", "Account", "Holder"),
        UiLanguage::PtBr => ("Agência", "Conta", "Titular"),
    };
    let kind = f.account_type.map(|t| match (lang, t) {
        (UiLanguage::En, Checking) => "checking",
        (UiLanguage::En, Savings) => "savings",
        (UiLanguage::En, Payment) => "payment",
        (UiLanguage::En, Other) => "other",
        (UiLanguage::PtBr, Checking) => "corrente",
        (UiLanguage::PtBr, Savings) => "poupança",
        (UiLanguage::PtBr, Payment) => "pagamento",
        (UiLanguage::PtBr, Other) => "outra",
    });
    let account_line = f.account.as_ref().map(|a| {
        let number = join(
            a.expose(),
            f.account_digit.as_ref().map(SecretString::expose),
        );
        match kind {
            Some(k) => format!("{account} {number} ({k})"),
            None => format!("{account} {number}"),
        }
    });
    let lines: Vec<String> = [
        f.bank.clone(),
        f.branch
            .as_deref()
            .map(|b| format!("{branch} {}", join(b, f.branch_digit.as_deref()))),
        account_line,
        f.holder_name
            .as_ref()
            .map(|h| format!("{holder} {}", h.expose())),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!lines.is_empty()).then(|| SecretString::new(lines.join("\n")))
}

fn copy_value(f: &BankAccountFields, field: CopyField, lang: UiLanguage) -> Option<SecretString> {
    match field {
        CopyField::Bank => f.bank.clone().map(SecretString::new),
        CopyField::HolderName => f.holder_name.clone(),
        CopyField::Details => details(f, lang),
        CopyField::Branch => Some(SecretString::new(join(
            f.branch.as_deref()?,
            f.branch_digit.as_deref(),
        ))),
        CopyField::Account => Some(SecretString::new(join(
            f.account.as_ref()?.expose(),
            f.account_digit.as_ref().map(SecretString::expose),
        ))),
        CopyField::Password => f.password.clone(),
    }
}

#[tauri::command]
pub fn reveal_bank(state: State<'_, AppState>, id: Uuid) -> CmdResult<BankView> {
    state.touch();
    let f = state.vault()?.bank_fields(id)?;
    Ok(BankView {
        has_account: f.account.is_some(),
        has_password: f.password.is_some(),
        bank: f.bank,
        holder_name: f.holder_name,
        account_type: f.account_type,
        branch: f.branch,
        branch_digit: f.branch_digit,
        notes: f.notes,
        sections: custom_field::views(f.sections),
    })
}

#[tauri::command]
pub fn reveal_bank_field(
    state: State<'_, AppState>,
    id: Uuid,
    field: RevealField,
) -> CmdResult<SecretString> {
    state.touch();
    let f = state.vault()?.bank_fields(id)?;
    Ok(reveal_value(&f, field).ok_or(Error::NotFound)?)
}

/// Copy one bank value, or the transfer details in `lang` (`"en"` or
/// `"pt-BR"`). Read here and handed to the clipboard, cleared after the
/// vault's delay; it does not pass through the renderer.
#[tauri::command]
pub fn copy_bank_field(
    state: State<'_, AppState>,
    id: Uuid,
    field: CopyField,
    lang: String,
) -> CmdResult<CopyResult> {
    let lang = UiLanguage::parse(&lang)?;
    copy_from_vault(&state, |v| {
        copy_value(&v.bank_fields(id)?, field, lang).ok_or(Error::NotFound)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, serde_json::Error> {
        serde_json::from_value(serde_json::Value::String(text.into()))
    }

    fn fields(branch_digit: Option<&str>, account_digit: Option<&str>) -> BankAccountFields {
        BankAccountFields {
            branch: Some("1234".into()),
            branch_digit: branch_digit.map(Into::into),
            account: Some(SecretString::from("12345")),
            account_digit: account_digit.map(SecretString::from),
            password: Some(SecretString::from("pw")),
            ..Default::default()
        }
    }

    #[test]
    fn only_the_account_digit_and_password_can_be_revealed() {
        assert_eq!(parse("account").ok(), Some(RevealField::Account));
        assert_eq!(parse("accountDigit").ok(), Some(RevealField::AccountDigit));
        assert_eq!(parse("password").ok(), Some(RevealField::Password));
        for bad in ["holderName", "branch", "bank", "notes", "Account", ""] {
            assert!(parse::<RevealField>(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn only_the_listed_values_can_be_copied() {
        assert_eq!(parse("bank").ok(), Some(CopyField::Bank));
        assert_eq!(parse("holderName").ok(), Some(CopyField::HolderName));
        assert_eq!(parse("branch").ok(), Some(CopyField::Branch));
        assert_eq!(parse("account").ok(), Some(CopyField::Account));
        assert_eq!(parse("password").ok(), Some(CopyField::Password));
        assert_eq!(parse("details").ok(), Some(CopyField::Details));
        for bad in ["accountDigit", "accountType", "branchDigit", "notes", ""] {
            assert!(parse::<CopyField>(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn copies_join_the_check_digit() {
        let f = fields(Some("X"), Some("6"));
        let copied = |field| copy_value(&f, field, UiLanguage::En).map(|s| s.expose().to_owned());
        assert_eq!(copied(CopyField::Branch).as_deref(), Some("1234-X"));
        assert_eq!(copied(CopyField::Account).as_deref(), Some("12345-6"));
        assert_eq!(copied(CopyField::Password).as_deref(), Some("pw"));
        let f = fields(None, None);
        assert_eq!(
            copy_value(&f, CopyField::Branch, UiLanguage::En)
                .unwrap()
                .expose(),
            "1234"
        );
        assert_eq!(
            copy_value(&f, CopyField::Account, UiLanguage::En)
                .unwrap()
                .expose(),
            "12345"
        );
    }

    #[test]
    fn details_list_every_part_in_the_ui_language() {
        let mut f = fields(Some("5"), Some("6"));
        f.bank = Some("Itaú".into());
        f.holder_name = Some(SecretString::from("Maria Silva"));
        f.account_type = Some(BankAccountType::Checking);
        let text = |lang| details(&f, lang).unwrap().expose().to_owned();
        assert_eq!(
            text(UiLanguage::PtBr),
            "Itaú\nAgência 1234-5\nConta 12345-6 (corrente)\nTitular Maria Silva"
        );
        assert_eq!(
            text(UiLanguage::En),
            "Itaú\nBranch 1234-5\nAccount 12345-6 (checking)\nHolder Maria Silva"
        );
    }

    #[test]
    fn details_leave_out_missing_parts() {
        let f = BankAccountFields {
            account: Some(SecretString::from("98765")),
            ..Default::default()
        };
        assert_eq!(
            details(&f, UiLanguage::PtBr).unwrap().expose(),
            "Conta 98765"
        );
        let f = BankAccountFields {
            bank: Some("Nubank".into()),
            branch: Some("0001".into()),
            ..Default::default()
        };
        assert_eq!(
            details(&f, UiLanguage::En).unwrap().expose(),
            "Nubank\nBranch 0001"
        );
        assert!(details(&BankAccountFields::default(), UiLanguage::En).is_none());
    }

    #[test]
    fn missing_values_are_absent() {
        let f = BankAccountFields::default();
        assert!(copy_value(&f, CopyField::Branch, UiLanguage::En).is_none());
        assert!(copy_value(&f, CopyField::Account, UiLanguage::En).is_none());
        assert!(reveal_value(&f, RevealField::AccountDigit).is_none());
        assert!(copy_value(&f, CopyField::Bank, UiLanguage::En).is_none());
        assert!(copy_value(&f, CopyField::HolderName, UiLanguage::En).is_none());
    }
}
