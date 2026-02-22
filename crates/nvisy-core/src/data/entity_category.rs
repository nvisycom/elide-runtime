//! Shared entity category tag.
//!
//! [`EntityCategory`] classifies detected sensitive data into broad
//! categories used by both detection and pattern matching crates.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Category of sensitive data an entity belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, EnumString)]
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
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
    #[strum(default)]
    Custom(String),
}
