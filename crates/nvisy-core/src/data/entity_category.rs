//! Shared entity category tag.
//!
//! [`EntityCategory`] classifies detected sensitive data into broad
//! categories used by both detection and pattern matching crates.

use serde::{Deserialize, Serialize};

/// Category of sensitive data an entity belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, strum::Display, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EntityCategory {
    /// Personally Identifiable Information (names, SSNs, addresses, etc.).
    Pii,
    /// Protected Health Information (HIPAA-regulated data).
    Phi,
    /// Financial data (credit card numbers, bank accounts, etc.).
    Financial,
    /// Secrets and credentials (API keys, passwords, tokens).
    Credentials,
    /// Legal documents and privileged communications.
    Legal,
    /// Biometric data (fingerprints, iris scans, voiceprints).
    Biometric,
    /// User-defined or plugin-specific category.
    #[strum(to_string = "{0}")]
    Custom(String),
}

impl EntityCategory {
    /// Parse a lowercase slug into an [`EntityCategory`].
    ///
    /// Recognises `"pii"`, `"phi"`, `"financial"`, `"credentials"`,
    /// `"legal"`, and `"biometric"`.  Anything else becomes
    /// [`Custom`](Self::Custom).
    pub fn from_slug(s: &str) -> Self {
        match s {
            "pii" => Self::Pii,
            "phi" => Self::Phi,
            "financial" => Self::Financial,
            "credentials" => Self::Credentials,
            "legal" => Self::Legal,
            "biometric" => Self::Biometric,
            other => Self::Custom(other.to_string()),
        }
    }
}
