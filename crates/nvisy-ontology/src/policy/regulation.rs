//! Regulatory framework identifiers.

use serde::{Deserialize, Serialize};

/// A compliance regulation or framework that a policy targets.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RegulationKind {
    /// Health Insurance Portability and Accountability Act.
    Hipaa,
    /// General Data Protection Regulation (EU).
    Gdpr,
    /// California Consumer Privacy Act.
    Ccpa,
    /// Payment Card Industry Data Security Standard.
    PciDss,
    /// Criminal Justice Information Services Security Policy.
    Cjis,
    /// Family Educational Rights and Privacy Act.
    Ferpa,
    /// Sarbanes-Oxley Act.
    Sox,
    /// User-defined regulation or framework.
    Custom(String),
}
