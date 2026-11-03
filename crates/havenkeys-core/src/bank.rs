//! The Bank Account item: bank, holder, account type, branch and account
//! numbers with their check digits (spec 2026-11-03-bank-account §4.2).
//!
//! The holder, account, digits, password and notes are [`SecretString`]s:
//! wiped on drop, never printed. Only the bank, the branch and the last
//! digits of the account go into the overview, for the list and search.
//! Cleanup lives here so every client stores the same text.

use crate::custom_field::FieldSection;
use crate::error::{Error, Result};
use crate::model::{check_notes, check_password, AppBinding, SecretUpdate};
use crate::secret::SecretString;
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::Zeroize;

pub const MAX_BANK_CHARS: usize = 64;
pub const MAX_HOLDER_CHARS: usize = 256;
pub const MAX_BRANCH_DIGITS: usize = 8;
pub const MAX_ACCOUNT_DIGITS: usize = 20;
pub const MAX_ACCOUNT_DIGIT_CHARS: usize = 2;
/// A shorter account shows no last digits: they would be most of it.
const MIN_DIGITS_FOR_LAST3: usize = 4;

const BAD_TEXT_MSG: &str = "bank or holder name is too long or contains control characters";
const BAD_NUMBER_MSG: &str = "branch and account must be digits, within their length limits";
const BAD_DIGIT_MSG: &str = "check digit must be 0-9 or X, within its length limit";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BankAccountType {
    Checking,
    Savings,
    Payment,
    Other,
}

/// The stored secret part of a bank account. Digits only in the numbers.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BankAccountFields {
    #[serde(default)]
    pub bank: Option<String>,
    #[serde(default)]
    pub holder_name: Option<SecretString>,
    #[serde(default)]
    pub account_type: Option<BankAccountType>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub branch_digit: Option<String>,
    #[serde(default)]
    pub account: Option<SecretString>,
    #[serde(default)]
    pub account_digit: Option<SecretString>,
    #[serde(default)]
    pub password: Option<SecretString>,
    #[serde(default)]
    pub notes: Option<SecretString>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<FieldSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub app_bindings: Vec<AppBinding>,
}

impl fmt::Debug for BankAccountFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BankAccountFields(<redacted>)")
    }
}

/// A bank account as the editor sends it. The account and its digit are
/// keep/set/clear, so editing never needs to read them. The password, notes,
/// URLs and sections come from the item's own fields, as for a login.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BankAccountInput {
    #[serde(default)]
    pub bank: Option<String>,
    #[serde(default)]
    pub holder_name: Option<SecretString>,
    #[serde(default)]
    pub account_type: Option<BankAccountType>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub branch_digit: Option<String>,
    #[serde(default)]
    pub account: SecretUpdate,
    #[serde(default)]
    pub account_digit: SecretUpdate,
}

impl fmt::Debug for BankAccountInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BankAccountInput(<redacted>)")
    }
}

/// What the list and search may show without a reveal.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankSummary {
    pub bank: Option<String>,
    pub branch: Option<String>,
    /// The last 3 characters of account and digit, `-` before the digit: `45-6`.
    pub account_last3: Option<String>,
}

impl fmt::Debug for BankSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BankSummary(<redacted>)")
    }
}

impl Drop for BankSummary {
    fn drop(&mut self) {
        self.account_last3.zeroize();
    }
}

/// The numbers a fill or copy needs; only an item with a branch and an
/// account has them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankValues {
    pub branch: String,
    pub branch_digit: Option<String>,
    pub account: SecretString,
    pub account_digit: Option<SecretString>,
}

impl fmt::Debug for BankValues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BankValues(<redacted>)")
    }
}

/// Drop spaces and dots, and upper-case an `x`.
fn compact(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_whitespace() && *c != '.')
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// A check digit: up to `max` of `0-9X`; empty is absent.
pub(crate) fn clean_digit(raw: &str, max: usize) -> Result<Option<String>> {
    let d = compact(raw);
    if d.is_empty() {
        return Ok(None);
    }
    if d.len() > max || !d.bytes().all(|b| b.is_ascii_digit() || b == b'X') {
        return Err(Error::InvalidInput(BAD_DIGIT_MSG));
    }
    Ok(Some(d))
}

/// A branch or account number: 1 to `max_digits` digits, with a trailing
/// `-D` split off as its check digit (up to `digit_max` characters).
pub(crate) fn clean_number(
    raw: &str,
    max_digits: usize,
    digit_max: usize,
) -> Result<(Option<String>, Option<String>)> {
    let c = compact(raw);
    if c.is_empty() {
        return Ok((None, None));
    }
    let (number, digit) = match c.rsplit_once('-') {
        Some((n, d)) => (n, clean_digit(d, digit_max)?),
        None => (c.as_str(), None),
    };
    if number.is_empty() || number.len() > max_digits || !number.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(Error::InvalidInput(BAD_NUMBER_MSG));
    }
    Ok((Some(number.to_owned()), digit))
}

/// Trimmed text up to `max` characters, no control characters; blank is absent.
fn clean_text(raw: &str, max: usize) -> Result<Option<String>> {
    let t = raw.trim();
    if t.chars().count() > max || t.chars().any(char::is_control) {
        return Err(Error::InvalidInput(BAD_TEXT_MSG));
    }
    Ok((!t.is_empty()).then(|| t.to_owned()))
}

/// A number and its digit, which may come split off the number or separate.
fn clean_pair(
    number: Option<&str>,
    digit: Option<&str>,
    max_digits: usize,
    digit_max: usize,
) -> Result<(Option<String>, Option<String>)> {
    let (number, split) = clean_number(number.unwrap_or_default(), max_digits, digit_max)?;
    let own = clean_digit(digit.unwrap_or_default(), digit_max)?;
    if split.is_some() && own.is_some() {
        return Err(Error::InvalidInput(BAD_DIGIT_MSG));
    }
    Ok((number, split.or(own)))
}

impl BankAccountFields {
    /// Validate and normalize every value.
    pub fn clean(self) -> Result<Self> {
        let (branch, branch_digit) = clean_pair(
            self.branch.as_deref(),
            self.branch_digit.as_deref(),
            MAX_BRANCH_DIGITS,
            1,
        )?;
        let (account, account_digit) = clean_pair(
            self.account.as_ref().map(SecretString::expose),
            self.account_digit.as_ref().map(SecretString::expose),
            MAX_ACCOUNT_DIGITS,
            MAX_ACCOUNT_DIGIT_CHARS,
        )?;
        if let Some(p) = &self.password {
            check_password(p)?;
        }
        if let Some(n) = &self.notes {
            check_notes(n)?;
        }
        Ok(Self {
            bank: self
                .bank
                .as_deref()
                .map(|b| clean_text(b, MAX_BANK_CHARS))
                .transpose()?
                .flatten(),
            holder_name: self
                .holder_name
                .as_ref()
                .map(|h| clean_text(h.expose(), MAX_HOLDER_CHARS))
                .transpose()?
                .flatten()
                .map(SecretString::new),
            branch,
            branch_digit,
            account: account.map(SecretString::new),
            account_digit: account_digit.map(SecretString::new),
            ..self
        })
    }

    pub fn summary(&self) -> BankSummary {
        let account_last3 = self
            .account
            .as_ref()
            .map(SecretString::expose)
            .filter(|a| a.chars().count() >= MIN_DIGITS_FOR_LAST3)
            .map(|a| {
                // The last 3 characters of account + digit, `-` before the digit.
                let digit = self.account_digit.as_ref().map_or("", SecretString::expose);
                let keep = 3usize.saturating_sub(digit.chars().count());
                let skip = a.chars().count() - keep;
                let tail: String = a.chars().skip(skip).collect();
                if digit.is_empty() {
                    tail
                } else {
                    format!("{tail}-{digit}")
                }
            });
        BankSummary {
            bank: self.bank.clone(),
            branch: self.branch.clone(),
            account_last3,
        }
    }

    /// `None` without a branch or an account.
    pub fn values(&self) -> Option<BankValues> {
        Some(BankValues {
            branch: self.branch.clone()?,
            branch_digit: self.branch_digit.clone(),
            account: self.account.clone()?,
            account_digit: self.account_digit.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> SecretString {
        SecretString::from(v)
    }

    fn some(v: &str) -> Option<String> {
        Some(v.to_owned())
    }

    #[test]
    fn clean_number_splits_and_normalizes() {
        assert_eq!(
            clean_number("1234-5", 8, 1).unwrap(),
            (some("1234"), some("5"))
        );
        assert_eq!(
            clean_number(" 12.345-x ", 20, 2).unwrap(),
            (some("12345"), some("X"))
        );
        assert_eq!(clean_number("   ", 8, 1).unwrap(), (None, None));
        assert_eq!(clean_number("1234", 8, 1).unwrap(), (some("1234"), None));
    }

    #[test]
    fn clean_number_rejects_bad_input() {
        for raw in ["12a", "123456789", "-5", "12-34"] {
            assert!(
                matches!(clean_number(raw, 8, 1), Err(Error::InvalidInput(_))),
                "{raw}"
            );
        }
    }

    #[test]
    fn clean_digit_limits() {
        assert!(clean_digit("Y", 1).is_err());
        assert!(clean_digit("XX", 1).is_err());
        assert_eq!(clean_digit("0X", 2).unwrap(), some("0X"));
        assert_eq!(clean_digit(" ", 1).unwrap(), None);
    }

    #[test]
    fn summary_shows_last_three_with_digit() {
        let mut f = BankAccountFields {
            account: Some(s("12345")),
            account_digit: Some(s("6")),
            ..Default::default()
        };
        assert_eq!(f.summary().account_last3.as_deref(), Some("45-6"));
        f.account_digit = None;
        f.account = Some(s("98765"));
        assert_eq!(f.summary().account_last3.as_deref(), Some("765"));
        f.account = Some(s("123"));
        assert_eq!(f.summary().account_last3, None);
        f.account = Some(s("12345"));
        f.account_digit = Some(s("66"));
        assert_eq!(f.summary().account_last3.as_deref(), Some("5-66"));
        // Not cleaned, not ASCII: must not panic.
        f.account = Some(s("ééééé"));
        assert_eq!(f.summary().account_last3.as_deref(), Some("é-66"));
    }

    #[test]
    fn clean_normalizes_every_field() {
        let f = BankAccountFields {
            bank: Some("  Itaú ".into()),
            holder_name: Some(s("   ")),
            branch: Some("1234-5".into()),
            account: Some(s("12.345-x")),
            ..Default::default()
        }
        .clean()
        .unwrap();
        assert_eq!(f.bank.as_deref(), Some("Itaú"));
        assert!(f.holder_name.is_none());
        assert_eq!(f.branch_digit.as_deref(), Some("5"));
        assert_eq!(f.account.unwrap().expose(), "12345");
        assert_eq!(f.account_digit.unwrap().expose(), "X");
    }

    #[test]
    fn clean_rejects_control_characters_and_double_digits() {
        let bad_bank = BankAccountFields {
            bank: Some("a\nb".into()),
            ..Default::default()
        };
        assert!(bad_bank.clean().is_err());
        let two_digits = BankAccountFields {
            branch: Some("1234-5".into()),
            branch_digit: Some("6".into()),
            ..Default::default()
        };
        assert!(two_digits.clean().is_err());
    }

    #[test]
    fn values_need_branch_and_account() {
        let mut f = BankAccountFields {
            branch: Some("1234".into()),
            ..Default::default()
        };
        assert!(f.values().is_none());
        f.account = Some(s("12345"));
        assert_eq!(f.values().unwrap().account.expose(), "12345");
    }

    #[test]
    fn debug_prints_no_values() {
        let f = BankAccountFields {
            bank: Some("Itaú".into()),
            holder_name: Some(s("Maria Souza")),
            branch: Some("1234".into()),
            account: Some(s("98765")),
            password: Some(s("hunter2")),
            ..Default::default()
        };
        for text in [
            format!("{f:?}"),
            format!("{:?}", f.summary()),
            format!("{:?}", f.values()),
        ] {
            for v in ["Itaú", "Maria", "1234", "98765", "hunter2"] {
                assert!(!text.contains(v), "{text}");
            }
        }
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(serde_json::from_str::<BankAccountFields>(r#"{"pin":"1"}"#).is_err());
        assert!(serde_json::from_str::<BankAccountInput>(r#"{"pin":"1"}"#).is_err());
        let ok: BankAccountInput =
            serde_json::from_str(r#"{"accountType":"savings","account":{"op":"clear"}}"#).unwrap();
        assert_eq!(ok.account_type, Some(BankAccountType::Savings));
    }
}
