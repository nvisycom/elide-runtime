//! [`LanguageTagSchema`]: wire shape for
//! [`elide_core::primitive::LanguageTag`].

use elide_core::primitive::LanguageTag;
use elide_core::{Error, ErrorKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Wire-shape proxy for [`elide_core::primitive::LanguageTag`].
///
/// A BCP 47 tag string (e.g. `"en"`, `"de-CH"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[schemars(rename = "LanguageTag", transparent)]
pub struct LanguageTagSchema(pub String);

impl TryFrom<LanguageTagSchema> for LanguageTag {
    type Error = Error;

    fn try_from(s: LanguageTagSchema) -> Result<Self, Self::Error> {
        LanguageTag::parse(s.0).map_err(|e| Error::new(ErrorKind::Validation, e))
    }
}

impl From<LanguageTag> for LanguageTagSchema {
    fn from(t: LanguageTag) -> Self {
        Self(t.as_str().to_owned())
    }
}
