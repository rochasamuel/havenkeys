//! Where a request came from, roughly, for the pairing confirmation
//! (spec 2026-10-03-phone-approved-sign-in §5.3). Read from a local MaxMind
//! format database (DB-IP Lite or GeoLite2) the operator downloads; no third
//! party is ever called. Without one, nothing is located.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::Path;

pub struct Locator(maxminddb::Reader<Vec<u8>>);

impl std::fmt::Debug for Locator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Locator")
    }
}

#[derive(Deserialize)]
struct Record {
    #[serde(default)]
    city: Option<Named>,
    #[serde(default)]
    country: Option<Country>,
}

#[derive(Deserialize)]
struct Named {
    #[serde(default)]
    names: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct Country {
    #[serde(default)]
    iso_code: Option<String>,
}

impl Locator {
    pub fn open(path: &Path) -> Result<Self, String> {
        maxminddb::Reader::open_readfile(path)
            .map(Self)
            .map_err(|_| "the IP location database could not be read".to_string())
    }

    pub fn locate(&self, ip: &str) -> Option<String> {
        let addr: IpAddr = ip.parse().ok()?;
        let record: Record = self.0.lookup(addr).ok()?.decode().ok()??;
        let city = record.city.and_then(|c| c.names.get("en").cloned());
        let country = record.country.and_then(|c| c.iso_code);
        label(city.as_deref(), country.as_deref())
    }
}

/// "City, CC", or "CC". Text from the database is checked like any input:
/// a city with control characters or of absurd length is dropped, format
/// characters (bidi, zero-width) are removed from it (PA4), and the country
/// must be two ASCII letters.
fn label(city: Option<&str>, country: Option<&str>) -> Option<String> {
    let clean = |s: &str| {
        let s = crate::routes::auth::without_format_chars(s);
        let s = s.trim();
        (!s.is_empty() && s.chars().count() <= 64 && !s.chars().any(char::is_control))
            .then(|| s.to_string())
    };
    let country = country.filter(|c| c.len() == 2 && c.bytes().all(|b| b.is_ascii_alphabetic()))?;
    let country = country.to_ascii_uppercase();
    match city.and_then(clean) {
        Some(city) => Some(format!("{city}, {country}")),
        None => Some(country),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_database_is_an_error_without_the_path_in_it() {
        let err = Locator::open(Path::new("/nonexistent/secret-dir/db.mmdb")).unwrap_err();
        assert!(!err.contains("secret-dir"));
    }

    #[test]
    fn the_label_prefers_city_and_country_then_country() {
        assert_eq!(
            label(Some("São Paulo"), Some("BR")).as_deref(),
            Some("São Paulo, BR")
        );
        assert_eq!(label(None, Some("BR")).as_deref(), Some("BR"));
        assert_eq!(label(Some("Nowhere"), None), None);
        assert_eq!(label(Some("A\u{7}"), Some("BR")).as_deref(), Some("BR"));
    }

    /// PA4: no bidi or zero-width characters, and a real country code.
    #[test]
    fn the_label_drops_format_characters_and_odd_country_codes() {
        assert_eq!(
            label(Some("Lis\u{202E}bon\u{200B}"), Some("pt")).as_deref(),
            Some("Lisbon, PT")
        );
        assert_eq!(label(Some("\u{200B}"), Some("BR")).as_deref(), Some("BR"));
        assert_eq!(label(None, Some("BRA")), None);
        assert_eq!(label(None, Some("B\u{202E}")), None);
        assert_eq!(label(None, Some("1A")), None);
    }
}
