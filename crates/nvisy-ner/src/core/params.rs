//! [`NerParams`] — per-call hints passed alongside the input text
//! to a [`Backend`].
//!
//! [`Backend`]: super::Backend

use nvisy_ontology::entity::EntityKind;
use nvisy_ontology::primitive::LanguageTag;
use uuid::Uuid;

/// Per-call hints passed alongside the input text to a [`Backend`].
///
/// All fields are advisory — backends are free to ignore any of
/// them when their model doesn't expose the corresponding knob.
/// Packed into a struct so the trait stays stable as new hint
/// kinds are added.
///
/// [`Backend`]: super::Backend
#[derive(Debug, Default, Clone, Copy)]
pub struct NerParams<'a> {
    /// Caller-resolved language for the input. Multilingual models
    /// may ignore it; monolingual models may validate it against a
    /// configured allowlist and return a validation error when the
    /// hint disagrees.
    pub language: Option<&'a LanguageTag>,

    /// Entity kinds the caller is interested in. Backends with a
    /// zero-shot label vector (e.g. GLiNER) shape inference around
    /// this; backends with a fixed label set ignore it. The
    /// [`NerEngine`] post-filters output against the same allowlist
    /// either way.
    ///
    /// [`NerEngine`]: crate::NerEngine
    pub requested_kinds: Option<&'a [EntityKind]>,

    /// Per-call correlation id propagated to remote backends (as
    /// the `x-request-id` header on the Bento backend). When
    /// `None`, transports that need a request id generate a
    /// UUIDv7 themselves so every request is traceable.
    pub correlation_id: Option<Uuid>,
}

impl<'a> NerParams<'a> {
    /// Construct an empty params (no language hint, no kind
    /// filter, no correlation id).
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder-style setter for the language hint.
    pub fn with_language(mut self, language: &'a LanguageTag) -> Self {
        self.language = Some(language);
        self
    }

    /// Builder-style setter for the requested-kinds hint.
    pub fn with_requested_kinds(mut self, kinds: &'a [EntityKind]) -> Self {
        self.requested_kinds = Some(kinds);
        self
    }

    /// Builder-style setter for the correlation id.
    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}
