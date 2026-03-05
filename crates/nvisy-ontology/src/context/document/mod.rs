//! Document templates and handwritten signatures.

mod signature;
mod template;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub use signature::SignatureData;
pub use template::TemplateData;

/// Document-related reference variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocumentVariant {
    /// Reference document template for layout classification.
    Template(TemplateData),
    /// Handwritten signature for verification.
    Signature(SignatureData),
}
