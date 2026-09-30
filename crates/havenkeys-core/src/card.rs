//! The Card item: a payment card's holder, brand, number, verification
//! number and expiry (spec 2026-09-29-card-item).
//!
//! The number, verification number, holder name and notes are
//! [`SecretString`]s: wiped on drop, never printed. Only the brand, the last
//! four digits and the expiry go into the overview, for the list and search.
//! Brand detection and the Luhn check live here so the desktop editor and
//! (later) autofill agree with what is stored.

use crate::error::{Error, Result};
use crate::model::SecretUpdate;
use crate::secret::SecretString;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use zeroize::Zeroize;

pub const MAX_CARDHOLDER_CHARS: usize = 256;
pub const MIN_NUMBER_DIGITS: usize = 8;
pub const MAX_NUMBER_DIGITS: usize = 19;
pub const MIN_VERIFICATION_DIGITS: usize = 3;
pub const MAX_VERIFICATION_DIGITS: usize = 8;
pub const MAX_CARD_NOTES_BYTES: usize = 64 * 1024;
const MIN_EXPIRY_YEAR: u16 = 1970;
const MAX_EXPIRY_YEAR: u16 = 2099;
/// A shorter number shows no last four digits: they would be most of it.
const MIN_DIGITS_FOR_LAST4: usize = 12;

/// The card network. `Other` is a network HavenKeys has no logo for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardBrand {
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

impl CardBrand {
    /// The wire id (`"visa"`, `"unionpay"`, …).
    pub fn id(self) -> &'static str {
        match self {
            CardBrand::Visa => "visa",
            CardBrand::Mastercard => "mastercard",
            CardBrand::Amex => "amex",
            CardBrand::Elo => "elo",
            CardBrand::Hipercard => "hipercard",
            CardBrand::Diners => "diners",
            CardBrand::Discover => "discover",
            CardBrand::Jcb => "jcb",
            CardBrand::Unionpay => "unionpay",
            CardBrand::Maestro => "maestro",
            CardBrand::Other => "other",
        }
    }

    /// The network's own name; the default title of a card without one.
    pub fn display_name(self) -> &'static str {
        match self {
            CardBrand::Visa => "Visa",
            CardBrand::Mastercard => "Mastercard",
            CardBrand::Amex => "American Express",
            CardBrand::Elo => "Elo",
            CardBrand::Hipercard => "Hipercard",
            CardBrand::Diners => "Diners Club",
            CardBrand::Discover => "Discover",
            CardBrand::Jcb => "JCB",
            CardBrand::Unionpay => "UnionPay",
            CardBrand::Maestro => "Maestro",
            CardBrand::Other => "Card",
        }
    }
}

/// A card's expiry month. On the wire and at rest: `"YYYY-MM"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardExpiry {
    pub year: u16,
    pub month: u8,
}

const BAD_EXPIRY_MSG: &str = "expiry must be a month and a year from 1970 to 2099";

fn bad_expiry() -> Error {
    Error::InvalidInput(BAD_EXPIRY_MSG)
}

impl CardExpiry {
    pub fn new(year: u16, month: u8) -> Result<Self> {
        if !(MIN_EXPIRY_YEAR..=MAX_EXPIRY_YEAR).contains(&year) || !(1..=12).contains(&month) {
            return Err(bad_expiry());
        }
        Ok(Self { year, month })
    }

    /// Exactly `YYYY-MM`.
    pub fn parse(text: &str) -> Result<Self> {
        let b = text.as_bytes();
        // ASCII first: slicing below must not split a multi-byte character.
        if !text.is_ascii()
            || b.len() != 7
            || b[4] != b'-'
            || !text[..4]
                .bytes()
                .chain(text[5..].bytes())
                .all(|c| c.is_ascii_digit())
        {
            return Err(bad_expiry());
        }
        let year: u16 = text[..4].parse().map_err(|_| bad_expiry())?;
        let month: u8 = text[5..].parse().map_err(|_| bad_expiry())?;
        Self::new(year, month)
    }

    pub fn to_wire(self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }

    /// `MM/YYYY`, as shown and copied.
    pub fn display(self) -> String {
        format!("{:02}/{:04}", self.month, self.year)
    }
}

impl Serialize for CardExpiry {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_wire())
    }
}

impl<'de> Deserialize<'de> for CardExpiry {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        CardExpiry::parse(&text).map_err(|_| serde::de::Error::custom("invalid card expiry"))
    }
}

/// One issuer-prefix range: numbers whose first `lo.len()` digits are within
/// `lo..=hi` (equal-length digit strings, compared as text) belong to `brand`.
#[derive(Clone, Copy, Debug)]
pub struct IinRange {
    pub brand: CardBrand,
    pub lo: &'static str,
    pub hi: &'static str,
}

/// Issuer prefixes, first match wins. Elo and Hipercard come first because
/// their ranges sit inside Visa's `4`, Maestro's `50`, Diners' `38`,
/// Discover's `65` and UnionPay's `62`.
///
/// Sources: braintree/credit-card-type `src/lib/card-types.ts` (MIT, commit
/// c50db7708ce3945a1cf00aca4220ee2356d2a580) for Elo, Visa, Mastercard,
/// American Express, Diners Club, Discover, JCB and UnionPay;
/// erikhenrique/bin-cc for Hipercard's 3841x0 and 6375xx/6376xx prefixes.
/// Maestro is limited to its dedicated prefixes (`50`, `56`–`58`, `6304`,
/// `67`) rather than braintree's catch-all `6`.
///
/// One row per line, never reformatted: the extension's parity test reads
/// these rows from this file.
#[rustfmt::skip]
pub const IIN_RANGES: &[IinRange] = &[
    IinRange { brand: CardBrand::Elo, lo: "401178", hi: "401179" },
    IinRange { brand: CardBrand::Elo, lo: "431274", hi: "431274" },
    IinRange { brand: CardBrand::Elo, lo: "438935", hi: "438935" },
    IinRange { brand: CardBrand::Elo, lo: "451416", hi: "451416" },
    IinRange { brand: CardBrand::Elo, lo: "457393", hi: "457393" },
    IinRange { brand: CardBrand::Elo, lo: "457631", hi: "457632" },
    IinRange { brand: CardBrand::Elo, lo: "504175", hi: "504175" },
    IinRange { brand: CardBrand::Elo, lo: "506699", hi: "506778" },
    IinRange { brand: CardBrand::Elo, lo: "509000", hi: "509999" },
    IinRange { brand: CardBrand::Elo, lo: "627780", hi: "627780" },
    IinRange { brand: CardBrand::Elo, lo: "636297", hi: "636297" },
    IinRange { brand: CardBrand::Elo, lo: "636368", hi: "636368" },
    IinRange { brand: CardBrand::Elo, lo: "650031", hi: "650033" },
    IinRange { brand: CardBrand::Elo, lo: "650035", hi: "650051" },
    IinRange { brand: CardBrand::Elo, lo: "650057", hi: "650081" },
    IinRange { brand: CardBrand::Elo, lo: "650405", hi: "650439" },
    IinRange { brand: CardBrand::Elo, lo: "650485", hi: "650538" },
    IinRange { brand: CardBrand::Elo, lo: "650541", hi: "650598" },
    IinRange { brand: CardBrand::Elo, lo: "650700", hi: "650718" },
    IinRange { brand: CardBrand::Elo, lo: "650720", hi: "650727" },
    IinRange { brand: CardBrand::Elo, lo: "650901", hi: "650978" },
    IinRange { brand: CardBrand::Elo, lo: "651652", hi: "651704" },
    IinRange { brand: CardBrand::Elo, lo: "655000", hi: "655019" },
    IinRange { brand: CardBrand::Elo, lo: "655021", hi: "655058" },
    IinRange { brand: CardBrand::Hipercard, lo: "606282", hi: "606282" },
    IinRange { brand: CardBrand::Hipercard, lo: "384100", hi: "384100" },
    IinRange { brand: CardBrand::Hipercard, lo: "384140", hi: "384140" },
    IinRange { brand: CardBrand::Hipercard, lo: "384160", hi: "384160" },
    IinRange { brand: CardBrand::Hipercard, lo: "637095", hi: "637095" },
    IinRange { brand: CardBrand::Hipercard, lo: "637568", hi: "637568" },
    IinRange { brand: CardBrand::Hipercard, lo: "637599", hi: "637599" },
    IinRange { brand: CardBrand::Hipercard, lo: "637609", hi: "637609" },
    IinRange { brand: CardBrand::Hipercard, lo: "637612", hi: "637612" },
    IinRange { brand: CardBrand::Visa, lo: "4", hi: "4" },
    IinRange { brand: CardBrand::Mastercard, lo: "51", hi: "55" },
    IinRange { brand: CardBrand::Mastercard, lo: "2221", hi: "2720" },
    IinRange { brand: CardBrand::Amex, lo: "34", hi: "34" },
    IinRange { brand: CardBrand::Amex, lo: "37", hi: "37" },
    IinRange { brand: CardBrand::Diners, lo: "300", hi: "305" },
    IinRange { brand: CardBrand::Diners, lo: "3095", hi: "3095" },
    IinRange { brand: CardBrand::Diners, lo: "36", hi: "36" },
    IinRange { brand: CardBrand::Diners, lo: "38", hi: "39" },
    IinRange { brand: CardBrand::Discover, lo: "6011", hi: "6011" },
    IinRange { brand: CardBrand::Discover, lo: "644", hi: "649" },
    IinRange { brand: CardBrand::Discover, lo: "65", hi: "65" },
    IinRange { brand: CardBrand::Jcb, lo: "3528", hi: "3589" },
    IinRange { brand: CardBrand::Unionpay, lo: "62", hi: "62" },
    IinRange { brand: CardBrand::Unionpay, lo: "8100", hi: "8171" },
    IinRange { brand: CardBrand::Maestro, lo: "50", hi: "50" },
    IinRange { brand: CardBrand::Maestro, lo: "56", hi: "58" },
    IinRange { brand: CardBrand::Maestro, lo: "6304", hi: "6304" },
    IinRange { brand: CardBrand::Maestro, lo: "67", hi: "67" },
];

/// The network of a number (digits only), from as many leading digits as
/// it has: `"4"` is Visa, `"401178"` Elo. `None` when nothing matches or the
/// text is not all digits.
pub fn detect_brand(digits: &str) -> Option<CardBrand> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    IIN_RANGES
        .iter()
        .find(|r| {
            let n = r.lo.len();
            digits.len() >= n && {
                let prefix = &digits[..n];
                prefix >= r.lo && prefix <= r.hi
            }
        })
        .map(|r| r.brand)
}

/// The Luhn check digit. Reported to the user, never enforced.
pub fn check_digit_ok(digits: &str) -> bool {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let sum: u32 = digits
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, b)| {
            let d = u32::from(b - b'0');
            if i % 2 == 1 {
                let x = d * 2;
                if x > 9 {
                    x - 9
                } else {
                    x
                }
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

const BAD_NUMBER_MSG: &str = "card number must be 8 to 19 digits";
const BAD_VERIFICATION_MSG: &str = "verification number must be 3 to 8 digits";
const BAD_HOLDER_MSG: &str = "cardholder name is too long or contains control characters";

/// Spaces and `-` removed; then 8–19 ASCII digits.
pub fn clean_number(raw: &str) -> Result<String> {
    let digits: String = raw.chars().filter(|c| *c != ' ' && *c != '-').collect();
    if !(MIN_NUMBER_DIGITS..=MAX_NUMBER_DIGITS).contains(&digits.len())
        || !digits.bytes().all(|b| b.is_ascii_digit())
    {
        let mut digits = digits;
        digits.zeroize();
        return Err(Error::InvalidInput(BAD_NUMBER_MSG));
    }
    Ok(digits)
}

/// Trimmed; then 3–8 ASCII digits.
pub fn clean_verification_number(raw: &str) -> Result<String> {
    let v = raw.trim();
    if !(MIN_VERIFICATION_DIGITS..=MAX_VERIFICATION_DIGITS).contains(&v.len())
        || !v.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(Error::InvalidInput(BAD_VERIFICATION_MSG));
    }
    Ok(v.to_owned())
}

/// A card's values. `brand` is the user's choice only; `None` means
/// "detect from the number" (see [`CardFields::effective_brand`]).
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardFields {
    #[serde(default)]
    pub cardholder_name: Option<SecretString>,
    #[serde(default)]
    pub brand: Option<CardBrand>,
    /// Digits only.
    #[serde(default)]
    pub number: Option<SecretString>,
    #[serde(default)]
    pub verification_number: Option<SecretString>,
    #[serde(default)]
    pub expiry: Option<CardExpiry>,
    #[serde(default)]
    pub notes: Option<SecretString>,
}

impl fmt::Debug for CardFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CardFields(<redacted>)")
    }
}

/// A single card value the desktop may copy or reveal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardField {
    CardholderName,
    Number,
    VerificationNumber,
    /// Copied as `MM/YYYY`.
    Expiry,
}

impl CardFields {
    /// Validate and normalize every value.
    pub fn clean(self) -> Result<Self> {
        let cardholder_name = match self.cardholder_name {
            Some(n) if n.expose().trim().is_empty() => None,
            Some(n) => {
                let t = n.expose().trim();
                if t.chars().count() > MAX_CARDHOLDER_CHARS || t.chars().any(char::is_control) {
                    return Err(Error::InvalidInput(BAD_HOLDER_MSG));
                }
                Some(SecretString::from(t))
            }
            None => None,
        };
        let number = match self.number {
            Some(n) if n.expose().trim().is_empty() => None,
            Some(n) => Some(SecretString::new(clean_number(n.expose())?)),
            None => None,
        };
        let verification_number = match self.verification_number {
            Some(v) if v.expose().trim().is_empty() => None,
            Some(v) => Some(SecretString::new(clean_verification_number(v.expose())?)),
            None => None,
        };
        let notes = match self.notes {
            Some(n) if n.expose().trim().is_empty() => None,
            Some(n) if n.expose().len() > MAX_CARD_NOTES_BYTES => {
                return Err(Error::InvalidInput("notes are too long"))
            }
            Some(n)
                if n.expose()
                    .chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\r')) =>
            {
                return Err(Error::InvalidInput("notes contain control characters"))
            }
            other => other,
        };
        Ok(Self {
            cardholder_name,
            brand: self.brand,
            number,
            verification_number,
            expiry: self.expiry,
            notes,
        })
    }

    /// The user's brand, else the one the number's prefix names, else `Other`.
    pub fn effective_brand(&self) -> CardBrand {
        self.brand
            .or_else(|| self.number.as_ref().and_then(|n| detect_brand(n.expose())))
            .unwrap_or(CardBrand::Other)
    }

    /// What the overview keeps: brand, last four digits, expiry.
    pub fn summary(&self) -> CardSummary {
        let last4 = self
            .number
            .as_ref()
            .map(SecretString::expose)
            .filter(|n| n.len() >= MIN_DIGITS_FOR_LAST4)
            .map(|n| n[n.len() - 4..].to_owned());
        CardSummary {
            brand: self.effective_brand(),
            last4,
            expiry: self.expiry,
        }
    }

    /// One value, for copying or revealing.
    pub fn value(&self, field: CardField) -> Option<SecretString> {
        match field {
            CardField::CardholderName => self.cardholder_name.clone(),
            CardField::Number => self.number.clone(),
            CardField::VerificationNumber => self.verification_number.clone(),
            CardField::Expiry => self.expiry.map(|e| SecretString::new(e.display())),
        }
    }
}

/// The card part of the overview, decrypted at unlock for the list and search.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardSummary {
    pub brand: CardBrand,
    pub last4: Option<String>,
    pub expiry: Option<CardExpiry>,
}

impl fmt::Debug for CardSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CardSummary")
            .field("brand", &self.brand)
            .finish_non_exhaustive()
    }
}

impl Drop for CardSummary {
    fn drop(&mut self) {
        self.last4.zeroize();
    }
}

/// A card as the editor sends it. The number and verification number are
/// keep/set/clear, so editing never needs to read them; the rest is sent in
/// full.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardInput {
    #[serde(default)]
    pub cardholder_name: Option<SecretString>,
    /// `None`: detect from the number.
    #[serde(default)]
    pub brand: Option<CardBrand>,
    #[serde(default)]
    pub number: SecretUpdate,
    #[serde(default)]
    pub verification_number: SecretUpdate,
    #[serde(default)]
    pub expiry: Option<CardExpiry>,
    #[serde(default)]
    pub notes: Option<SecretString>,
}

impl fmt::Debug for CardInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CardInput")
            .field("brand", &self.brand)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(prefix: &str) -> String {
        format!("{prefix:0<16}")
    }

    #[test]
    fn every_range_detects_its_own_brand_at_both_ends() {
        for r in IIN_RANGES {
            assert_eq!(r.lo.len(), r.hi.len(), "{:?} {}..{}", r.brand, r.lo, r.hi);
            assert!(r.lo <= r.hi, "{:?} {}..{}", r.brand, r.lo, r.hi);
            assert_eq!(detect_brand(&pad(r.lo)), Some(r.brand), "lo {}", r.lo);
            assert_eq!(detect_brand(&pad(r.hi)), Some(r.brand), "hi {}", r.hi);
        }
    }

    #[test]
    fn known_numbers() {
        for (n, b) in [
            ("4242424242424242", CardBrand::Visa),
            ("5555555555554444", CardBrand::Mastercard),
            ("2223003122003222", CardBrand::Mastercard),
            ("378282246310005", CardBrand::Amex),
            ("6011111111111117", CardBrand::Discover),
            ("3056930009020004", CardBrand::Diners),
            ("36227206271667", CardBrand::Diners),
            ("3566002020360505", CardBrand::Jcb),
            ("6200000000000005", CardBrand::Unionpay),
            ("6362970000457013", CardBrand::Elo),
            ("6062825624254001", CardBrand::Hipercard),
            ("6759649826438453", CardBrand::Maestro),
        ] {
            assert_eq!(detect_brand(n), Some(b), "{n}");
        }
    }

    #[test]
    fn overlapping_prefixes_pick_the_specific_brand() {
        assert_eq!(
            detect_brand(&pad("401178")),
            Some(CardBrand::Elo),
            "not Visa"
        );
        assert_eq!(
            detect_brand(&pad("506699")),
            Some(CardBrand::Elo),
            "not Maestro"
        );
        assert_eq!(
            detect_brand(&pad("650031")),
            Some(CardBrand::Elo),
            "not Discover"
        );
        assert_eq!(
            detect_brand(&pad("627780")),
            Some(CardBrand::Elo),
            "not UnionPay"
        );
        assert_eq!(
            detect_brand(&pad("384100")),
            Some(CardBrand::Hipercard),
            "not Diners"
        );
        assert_eq!(detect_brand(&pad("384200")), Some(CardBrand::Diners));
        assert_eq!(detect_brand(&pad("650030")), Some(CardBrand::Discover));
        assert_eq!(detect_brand(&pad("2200")), None, "Mir is not Mastercard");
    }

    #[test]
    fn partial_numbers_and_garbage() {
        assert_eq!(detect_brand("4"), Some(CardBrand::Visa));
        assert_eq!(
            detect_brand("40117"),
            Some(CardBrand::Visa),
            "too short to be Elo yet"
        );
        assert_eq!(detect_brand("401178"), Some(CardBrand::Elo));
        assert_eq!(detect_brand("5"), None);
        assert_eq!(detect_brand(""), None);
        assert_eq!(detect_brand("4a"), None);
        assert_eq!(detect_brand("9999999999999999"), None);
        assert_eq!(
            detect_brand("１２３"),
            None,
            "fullwidth digits are not digits"
        );
    }

    #[test]
    fn luhn() {
        assert!(check_digit_ok("4111111111111111"));
        assert!(check_digit_ok("5200828282828210"));
        assert!(check_digit_ok("378282246310005"));
        assert!(!check_digit_ok("4111111111111112"));
        assert!(!check_digit_ok(""));
        assert!(!check_digit_ok("4111 1111 1111 1111"));
    }

    #[test]
    fn clean_number_rules() {
        assert_eq!(
            clean_number(" 5200 8282-8282 8210 ").unwrap(),
            "5200828282828210"
        );
        assert_eq!(clean_number("12345678").unwrap(), "12345678");
        assert!(clean_number("1234567").is_err());
        assert!(clean_number(&"1".repeat(20)).is_err());
        assert!(clean_number("4111 1111 1111 111a").is_err());
        assert!(clean_number("4111.1111.1111.1111").is_err());
        let Error::InvalidInput(msg) = clean_number("41111111111111119999").unwrap_err() else {
            panic!("InvalidInput expected")
        };
        assert!(!msg.contains('4'), "the message never echoes the number");
    }

    #[test]
    fn verification_number_rules() {
        assert_eq!(clean_verification_number(" 123 ").unwrap(), "123");
        assert_eq!(clean_verification_number("12345678").unwrap(), "12345678");
        for bad in ["12", "123456789", "12a", "1 23", ""] {
            assert!(clean_verification_number(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn expiry_wire_format() {
        let e = CardExpiry::parse("2033-11").unwrap();
        assert_eq!((e.year, e.month), (2033, 11));
        assert_eq!(e.to_wire(), "2033-11");
        assert_eq!(e.display(), "11/2033");
        assert_eq!(serde_json::to_string(&e).unwrap(), "\"2033-11\"");
        for bad in [
            "2033-13",
            "2033-00",
            "1969-12",
            "2100-01",
            "33-11",
            "2033-1",
            "2033/11",
            "２０３３-11",
        ] {
            assert!(CardExpiry::parse(bad).is_err(), "{bad:?}");
            assert!(serde_json::from_str::<CardExpiry>(&format!("\"{bad}\"")).is_err());
        }
    }

    #[test]
    fn effective_brand_and_summary() {
        let f = CardFields {
            number: Some(SecretString::from("5200828282828210")),
            expiry: Some(CardExpiry::new(2033, 11).unwrap()),
            ..Default::default()
        };
        assert_eq!(f.effective_brand(), CardBrand::Mastercard);
        let s = f.summary();
        assert_eq!(s.brand, CardBrand::Mastercard);
        assert_eq!(s.last4.as_deref(), Some("8210"));

        let overridden = CardFields {
            brand: Some(CardBrand::Elo),
            ..f.clone()
        };
        assert_eq!(overridden.effective_brand(), CardBrand::Elo);

        let short = CardFields {
            number: Some(SecretString::from("12345678")),
            ..Default::default()
        };
        assert_eq!(short.summary().last4, None, "no last 4 under 12 digits");
        assert_eq!(CardFields::default().effective_brand(), CardBrand::Other);
    }

    #[test]
    fn debug_never_shows_values() {
        let f = CardFields {
            cardholder_name: Some(SecretString::from("Samuel S Rocha")),
            number: Some(SecretString::from("5200828282828210")),
            verification_number: Some(SecretString::from("987")),
            ..Default::default()
        };
        let text = format!("{f:?} {:?}", f.summary());
        for secret in ["Samuel", "5200", "8210", "987"] {
            assert!(!text.contains(secret), "{text}");
        }
        let input = CardInput {
            number: SecretUpdate::Set(SecretString::from("5200828282828210")),
            ..Default::default()
        };
        assert!(!format!("{input:?}").contains("5200"));
    }

    #[test]
    fn card_input_rejects_unknown_fields() {
        assert!(serde_json::from_str::<CardInput>(r#"{"pin":"1234"}"#).is_err());
        let ok: CardInput = serde_json::from_str(
            r#"{"cardholderName":"S","brand":"elo","number":{"op":"set","value":"5067"},"expiry":"2030-01"}"#,
        )
        .unwrap();
        assert_eq!(ok.brand, Some(CardBrand::Elo));
    }
}
