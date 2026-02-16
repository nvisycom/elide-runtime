//! Text rendering and redaction primitives.
//!
//! Provides byte-offset replacement, cell-level masking, and the
//! [`AsText`] / [`AsRedactableText`] traits that text-bearing handlers
//! implement to support redaction in a single call.
//!
//! # Traits
//!
//! [`AsText`] is the codec extension point: text format handlers
//! implement [`content`](AsText::content) and
//! [`replace_content`](AsText::replace_content) to read and write their
//! backing text.
//!
//! [`AsRedactableText`] adds a [`redact`](AsRedactableText::redact)
//! convenience method that resolves [`TextRedaction`] items into
//! byte-offset replacements. It is automatically implemented for every
//! type that implements [`AsText`].

mod mask;
mod replace;

pub use mask::mask_cell;

use replace::{apply_replacements, PendingReplacement};

use nvisy_core::error::Error;
use crate::render::output::TextRedactionOutput;

/// A located text redaction: pairs a byte range with a
/// [`TextRedactionOutput`] that carries the already-resolved replacement.
pub struct TextRedaction {
    /// Byte offset where the redacted span starts in the content.
    pub start: usize,
    /// Byte offset where the redacted span ends (exclusive) in the content.
    pub end: usize,
    /// The redaction output that carries the replacement value.
    pub output: TextRedactionOutput,
}

/// Trait for handlers that wrap text content.
///
/// Handlers implement [`content`](Self::content) and
/// [`replace_content`](Self::replace_content) to round-trip through
/// plain text. See [`AsRedactableText`] for the higher-level redaction
/// API.
pub trait AsText: Sized {
    /// Return the handler's full text content as a single string.
    fn content(&self) -> String;

    /// Build a new handler instance with the given text content.
    fn replace_content(&self, content: &str) -> Result<Self, Error>;
}

/// Extension trait that adds [`TextRedactionOutput`]-driven redaction
/// to any [`AsText`] implementor.
///
/// This trait is automatically implemented for every type that implements
/// [`AsText`] — handler authors only need to implement [`AsText`].
pub trait AsRedactableText: AsText {
    /// Apply a batch of text redactions, returning a new handler.
    ///
    /// Each [`TextRedaction`] identifies a byte range and a
    /// [`TextRedactionOutput`] whose replacement value is written into
    /// the content. Replacements are applied right-to-left so that byte
    /// offsets remain valid.
    fn redact(&self, redactions: &[TextRedaction]) -> Result<Self, Error> {
        if redactions.is_empty() {
            return self.replace_content(&self.content());
        }

        let content = self.content();
        let mut pending: Vec<PendingReplacement> = redactions
            .iter()
            .map(|r| {
                let value = r.output.replacement_value()
                    .unwrap_or_default()
                    .to_string();
                PendingReplacement {
                    start: r.start,
                    end: r.end,
                    value,
                }
            })
            .collect();

        let result = apply_replacements(&content, &mut pending);
        self.replace_content(&result)
    }
}

/// Blanket implementation: every [`AsText`] type gets [`AsRedactableText`] for free.
impl<T: AsText> AsRedactableText for T {}
