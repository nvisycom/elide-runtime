//! [`Hint<M>`]: uploader-supplied annotation region in modality-native
//! coordinates.
//!
//! A hint marks a span the uploader believes might contain a
//! sensitive entity (and optionally claims an entity kind and a
//! display name for it). Recognizers that support hint adjudication
//! — typically LLM-based ones — fold hints into their detection pass
//! so the model can confirm, relocate, or implicitly reject each one
//! alongside open-ended discovery. Recognizers that don't (pattern,
//! dictionary, generic NER backends) ignore [`RecognizerInput::hints`]
//! entirely.
//!
//! `Hint<M>` mirrors [`Entity<M>`] structurally: both carry a
//! modality-native coordinate. The difference is provenance — an
//! entity is a recognizer's *output* with a confidence and a trail;
//! a hint is a recognizer's *input* with no trail yet.
//!
//! [`Entity<M>`]: crate::entity::Entity
//! [`RecognizerInput::hints`]: super::RecognizerInput::hints

use crate::entity::EntityKind;
use crate::modality::Modality;

/// Uploader-supplied annotation region in modality-native
/// coordinates (`Text` byte range, `Image` bounding box, etc).
#[derive(Debug, Clone, PartialEq)]
pub struct Hint<M: Modality> {
    /// Uploader-supplied display name (optional). Recognizers that
    /// confirm or relocate this hint forward the name into the
    /// emitted entity's recognition trail step.
    pub name: Option<String>,
    /// Uploader-claimed entity kind (optional).
    pub entity_kind: Option<EntityKind>,
    /// Region in modality-native coordinates.
    pub location: M,
}

impl<M: Modality> Hint<M> {
    /// Construct a hint with only the location set; name and kind
    /// default to `None`.
    #[must_use]
    pub fn new(location: M) -> Self {
        Self {
            name: None,
            entity_kind: None,
            location,
        }
    }

    /// Attach an uploader-supplied display name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Attach an uploader-claimed entity kind.
    #[must_use]
    pub fn with_entity_kind(mut self, entity_kind: EntityKind) -> Self {
        self.entity_kind = Some(entity_kind);
        self
    }
}
