//! [ISO 3166-1 alpha-2] country code type.
//!
//! Thin wrapper around [`celes::Country`] that exposes only the
//! alpha-2 surface — alpha-3, numeric, and long-name forms are
//! reachable via [`CountryCode::into_inner`] for the rare consumer
//! that needs them.
//!
//! [ISO 3166-1 alpha-2]: https://en.wikipedia.org/wiki/ISO_3166-1_alpha-2

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::Error;

/// A validated [ISO 3166-1 alpha-2] country code.
///
/// Two-letter uppercase code identifying a country or region.
/// Construction accepts any case (`"us"`, `"US"`, `"uS"`) and
/// rejects anything that isn't a known ISO 3166-1 alpha-2 code.
///
/// # Examples
///
/// ```
/// use nvisy_core::primitive::CountryCode;
///
/// let us = CountryCode::new("us").unwrap();
/// assert_eq!(us.as_str(), "US");
///
/// assert!(CountryCode::new("USA").is_err());
/// assert!(CountryCode::new("XZ").is_err());
/// ```
///
/// [ISO 3166-1 alpha-2]: https://en.wikipedia.org/wiki/ISO_3166-1_alpha-2
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CountryCode(celes::Country);

impl CountryCode {
    /// Parse and validate a country code.
    ///
    /// Input is case-insensitive; the canonical form returned by
    /// [`as_str`] is uppercase.
    ///
    /// # Errors
    ///
    /// Returns a validation error when `code` is not a known
    /// ISO 3166-1 alpha-2 code.
    ///
    /// [`as_str`]: Self::as_str
    pub fn new(code: &str) -> Result<Self, Error> {
        celes::Country::from_alpha2(code).map(Self).map_err(|_| {
            Error::validation(
                format!("country code `{code}` is not a known ISO 3166-1 alpha-2 code"),
                "nvisy-core",
            )
        })
    }

    /// Return the canonical (uppercase) alpha-2 representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        self.0.alpha2
    }

    /// Borrow the underlying [`celes::Country`] for callers that
    /// need alpha-3, numeric, or the country's long name.
    #[must_use]
    pub fn into_inner(self) -> celes::Country {
        self.0
    }
}

impl fmt::Display for CountryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CountryCode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for CountryCode {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<CountryCode> for String {
    fn from(code: CountryCode) -> Self {
        code.as_str().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_uppercase_alpha2() {
        let us = CountryCode::new("US").unwrap();
        assert_eq!(us.as_str(), "US");
    }

    #[test]
    fn accepts_lowercase_alpha2_and_normalises() {
        let us = CountryCode::new("us").unwrap();
        assert_eq!(us.as_str(), "US");
    }

    #[test]
    fn accepts_mixed_case() {
        let gb = CountryCode::new("Gb").unwrap();
        assert_eq!(gb.as_str(), "GB");
    }

    #[test]
    fn rejects_alpha3_code() {
        assert!(CountryCode::new("USA").is_err());
        assert!(CountryCode::new("GBR").is_err());
    }

    #[test]
    fn rejects_unassigned_two_letter_code() {
        assert!(CountryCode::new("XZ").is_err());
        assert!(CountryCode::new("ZZ").is_err());
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(CountryCode::new("U").is_err());
        assert!(CountryCode::new("").is_err());
        assert!(CountryCode::new("USAB").is_err());
    }

    #[test]
    fn rejects_non_alpha() {
        assert!(CountryCode::new("U1").is_err());
        assert!(CountryCode::new("00").is_err());
    }

    #[test]
    fn equality_is_canonical() {
        // `us` and `US` should compare equal once parsed.
        let lower = CountryCode::new("us").unwrap();
        let upper = CountryCode::new("US").unwrap();
        assert_eq!(lower, upper);
    }

    #[test]
    fn serde_roundtrip_uppercase() {
        let us = CountryCode::new("us").unwrap();
        let json = serde_json::to_string(&us).unwrap();
        assert_eq!(json, "\"US\"");
        let back: CountryCode = serde_json::from_str(&json).unwrap();
        assert_eq!(back, us);
    }

    #[test]
    fn from_str_parses_alpha2() {
        let de: CountryCode = "DE".parse().unwrap();
        assert_eq!(de.as_str(), "DE");
    }

    #[test]
    fn display_writes_alpha2() {
        let fr = CountryCode::new("FR").unwrap();
        assert_eq!(format!("{fr}"), "FR");
    }
}
