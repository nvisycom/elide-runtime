//! [`Context`] — per-call hints passed alongside the input image
//! to a [`Backend`].
//!
//! [`Backend`]: super::Backend

use nvisy_core::primitive::LanguageTag;
use uuid::Uuid;

/// Per-call hints passed alongside the input image to a [`Backend`].
///
/// All fields are advisory — backends are free to ignore any of
/// them when their model doesn't expose the corresponding knob.
/// Packed into a struct so the trait stays stable as new hint
/// kinds are added.
///
/// Borrowed (`Context<'a>`) so call sites that already own the
/// underlying values hand them through without cloning.
///
/// [`Backend`]: super::Backend
#[derive(Debug, Default, Clone, Copy)]
pub struct Context<'a> {
    /// Caller-supplied language hint. Multilingual OCR engines may
    /// ignore it; engines with a per-language model variant use it
    /// to pick the right one.
    pub language: Option<&'a LanguageTag>,

    /// Per-call correlation id propagated to remote backends (as
    /// the `x-request-id` header on the Bento backend). When
    /// `None`, transports that need a request id generate a
    /// UUIDv7 themselves so every request is traceable.
    pub correlation_id: Option<Uuid>,
}

impl<'a> Context<'a> {
    /// Construct an empty context (no language hint, no correlation id).
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder-style setter for the language hint.
    pub fn with_language(mut self, language: &'a LanguageTag) -> Self {
        self.language = Some(language);
        self
    }

    /// Builder-style setter for the correlation id.
    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}
