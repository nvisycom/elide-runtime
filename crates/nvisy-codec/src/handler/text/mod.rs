//! Text-handler trait + supporting infrastructure.
//!
//! The trait, redaction shape, and `apply_text_redaction` helper
//! live here; concrete per-format implementations (TXT, JSON,
//! Markdown, HTML) live in `nvisy-formats`.

use nvisy_core::Error;
use nvisy_ontology::modality::Text;

use super::Handler;
use crate::document::LocationStream;
use crate::handler::Redactions;

mod apply;
mod boxed;
mod instruction;
mod text_data;

pub use self::apply::apply_text_redaction;
pub use self::boxed::BoxedTextHandler;
pub use self::instruction::{TextOutput, TextRedaction};
pub use self::text_data::TextData;

/// Capability trait for handlers that expose text content.
///
/// Handlers expose text content as a stream of [`Text`]s
/// (cheap, identity-only), with explicit `read` calls to fetch the
/// payload for any given location, and a `redact` call that applies a
/// batch of [`TextRedaction`]s grouped by location.
///
/// # Offset semantics
///
/// Byte offsets in [`Text`] are relative to the handler's
/// **serialized** form. For plain text this is identical to the
/// in-memory form; for JSON and CSV the offsets include formatting
/// characters (quotes, escapes, delimiters). Use [`read`] to extract
/// the logical value at a location rather than slicing the serialized
/// bytes directly.
///
/// [`read`]: TextHandler::read
#[async_trait::async_trait]
pub trait TextHandler: Handler {
    /// Async stream of [`Text`]s for this document, each
    /// tagged with the handler's [`ContentSource`].
    ///
    /// [`ContentSource`]: nvisy_core::content::ContentSource
    fn locations(&self) -> LocationStream<'_, Text>;

    /// Read the text content at the given location.
    ///
    /// Returns `None` if the location is out of bounds.
    async fn read(&self, location: &Text) -> Option<TextData>;

    /// Apply a single redaction at the given location, mutating in
    /// place. Implementations need not handle iteration or overlap —
    /// the provided [`redact`] feeds one `(location, redaction)` pair
    /// at a time.
    ///
    /// [`redact`]: TextHandler::redact
    async fn redact_at(&mut self, location: &Text, redaction: TextRedaction) -> Result<(), Error>;

    /// Apply every `(location, redaction)` pair in `redactions` to the
    /// handler in insertion order. The first error aborts the batch.
    ///
    /// The default loops [`redact_at`] in [`Redactions`] insertion
    /// order; handlers with ordering constraints (see
    /// [`AudioHandler::redact`]) override this default.
    ///
    /// [`redact_at`]: TextHandler::redact_at
    /// [`AudioHandler::redact`]: crate::handler::AudioHandler::redact
    async fn redact(&mut self, redactions: Redactions<Text, TextRedaction>) -> Result<(), Error> {
        for (location, redaction) in redactions {
            self.redact_at(&location, redaction).await?;
        }
        Ok(())
    }
}
