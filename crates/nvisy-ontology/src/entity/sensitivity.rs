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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
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

    #[test]
    fn ordering() {
        assert!(EntitySensitivity::Critical > EntitySensitivity::High);
        assert!(EntitySensitivity::High > EntitySensitivity::Medium);
        assert!(EntitySensitivity::Medium > EntitySensitivity::Low);
    }
}
