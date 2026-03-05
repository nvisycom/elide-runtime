//! Raw reference data for direct comparison against input.

mod credential;
mod image;
mod tag;
mod text;

pub use credential::CredentialData;
pub use image::ImageData;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub use tag::TagData;
pub use text::{TextData, TextEntry};

/// Direct comparison reference variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReferenceVariant {
    /// Names, identifiers, or phrases to match.
    Text(TextData),
    /// Keyword tag for classification and routing.
    Tag(TagData),
    /// API keys, tokens, or known secret patterns.
    Credential(CredentialData),
    /// Reference image for object/scene matching.
    Object(ImageData),
}
