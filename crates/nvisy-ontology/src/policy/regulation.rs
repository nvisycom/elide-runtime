//! Regulatory framework identifiers.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::Display;

/// A compliance regulation or framework that a policy targets.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Display, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
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
    #[strum(to_string = "{0}")]
    Custom(String),
}
