//! Raw reference data for direct comparison against input.

mod credential;
mod image;
mod tag;
mod text;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::credential::{CredentialData, CredentialKind};
pub use self::image::ImageData;
pub use self::tag::TagData;
pub use self::text::{TextData, TextEntry};

/// Direct comparison reference variants.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReferenceVariant {
    /// Names, identifiers, or phrases to match.
    Text(TextData),
    /// Keyword tag for classification and routing.
    Tag(TagData),
    /// API keys, tokens, or known secret patterns.
    Credential(CredentialData),
    /// Reference image for object/scene matching.
    Image(ImageData),
}
