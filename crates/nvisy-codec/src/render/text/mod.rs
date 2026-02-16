//! Text rendering and redaction primitives.
//!
//! Provides byte-offset replacement, cell-level masking, and the
//! [`AsText`] trait that text-bearing handlers implement to support
//! redaction in a single call.
//!
//! # Trait
//!
//! [`AsText`] is the main extension point: text format handlers implement
//! [`content`](AsText::content) and [`replace_content`](AsText::replace_content)
//! to read and write their backing text, and then get a
//! [`redact`](AsText::redact) convenience method for free via the default
//! implementation.
//!
//! # Sub-modules
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`replace`] | Byte-offset text replacement engine |
//! | [`mask`] | Cell-level masking and hashing utilities |

pub mod mask;
pub mod replace;

pub use mask::{hash_string, mask_cell};
pub use replace::{apply_replacements, PendingReplacement};

use nvisy_core::error::Error;
use nvisy_ontology::redaction::TextRedactionOutput;

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

/// Trait for handlers that hold redactable text content.
///
/// Mirrors [`AsImage`](super::image::AsImage) for the text modality.
/// Handlers implement [`content`](Self::content) and
/// [`replace_content`](Self::replace_content), and get
/// [`redact`](Self::redact) for free.
pub trait AsText: Sized {
    /// Return the handler's full text content as a single string.
    fn content(&self) -> String;

    /// Build a new handler instance with the given text content.
    fn replace_content(&self, content: &str) -> Result<Self, Error>;

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
