//! Data sensitivity level.
//!
//! [`EntitySensitivity`] ranks how sensitive a piece of detected data is,
//! independent of its category or kind.  This drives default redaction
//! behaviour — higher sensitivity means stricter handling.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// How sensitive a detected entity is.
///
/// Ordered from least to most sensitive.  Consumers can compare variants
/// directly (`Critical > High > Medium > Low`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
    Display, EnumString,
    Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EntitySensitivity {
    /// Public or quasi-public data (e.g. company names, URLs).
    Low,
    /// Data that can indirectly identify a person (e.g. age, postal code).
    Medium,
    /// Directly identifying data (e.g. full name, email, phone number).
    High,
    /// Data requiring the strictest protection (e.g. SSN, biometrics,
    /// credentials, payment cards).
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn ordering() {
        assert!(EntitySensitivity::Critical > EntitySensitivity::High);
        assert!(EntitySensitivity::High > EntitySensitivity::Medium);
        assert!(EntitySensitivity::Medium > EntitySensitivity::Low);
    }

    #[test]
    fn display_snake_case() {
        assert_eq!(EntitySensitivity::Critical.to_string(), "critical");
        assert_eq!(EntitySensitivity::Low.to_string(), "low");
    }

    #[test]
    fn parse_roundtrip() {
        let s = EntitySensitivity::from_str("high").unwrap();
        assert_eq!(s, EntitySensitivity::High);
    }

    #[test]
    fn serde_roundtrip() {
        let s = EntitySensitivity::Critical;
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"critical\"");
        let back: EntitySensitivity = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
