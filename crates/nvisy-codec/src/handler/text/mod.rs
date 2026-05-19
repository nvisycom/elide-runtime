//! Text-based format handlers.

use nvisy_core::Error;
use nvisy_ontology::entity::TextLocation;

use super::Handler;
use crate::document::LocationStream;
use crate::handler::Redactions;

mod apply;
#[cfg(feature = "html")]
mod html_handler;
#[cfg(feature = "html")]
mod html_loader;
mod instruction;
mod json_handler;
mod json_loader;
mod markdown_loader;
mod text_data;
mod text_handler;
mod txt_handler;
mod txt_loader;

pub(crate) use self::apply::apply_text_redaction;
#[cfg(feature = "html")]
pub use self::html_handler::{HtmlData, HtmlHandler};
#[cfg(feature = "html")]
pub use self::html_loader::{HtmlLoader, HtmlParams};
pub use self::instruction::{TextOutput, TextRedaction};
pub use self::json_handler::{JsonData, JsonHandler, JsonIndent};
pub use self::json_loader::{JsonLoader, JsonParams};
pub use self::markdown_loader::{MarkdownLoader, MarkdownParams};
pub use self::text_data::TextData;
pub use self::text_handler::BoxedTextHandler;
pub use self::txt_handler::TxtHandler;
pub use self::txt_loader::{TxtLoader, TxtParams};

/// Capability trait for handlers that expose text content.
///
/// Handlers expose text content as a stream of [`TextLocation`]s
/// (cheap, identity-only), with explicit `read` calls to fetch the
/// payload for any given location, and a `redact` call that applies a
/// batch of [`TextRedaction`]s grouped by location.
///
/// # Offset semantics
///
/// Byte offsets in [`TextLocation`] are relative to the handler's
/// **serialized** form. For plain text this is identical to the
/// in-memory form; for JSON and CSV the offsets include formatting
/// characters (quotes, escapes, delimiters). Use [`read`] to extract
/// the logical value at a location rather than slicing the serialized
/// bytes directly.
///
/// [`read`]: TextHandler::read
#[async_trait::async_trait]
pub trait TextHandler: Handler {
    /// Async stream of [`TextLocation`]s for this document, each
    /// tagged with the handler's [`ContentSource`].
    ///
    /// [`ContentSource`]: nvisy_core::content::ContentSource
    fn locations(&self) -> LocationStream<'_, TextLocation>;

    /// Read the text content at the given location.
    ///
    /// Returns `None` if the location is out of bounds.
    async fn read(&self, location: &TextLocation) -> Option<TextData>;

    /// Apply a single redaction at the given location, mutating in
    /// place. Implementations need not handle iteration or overlap —
    /// the provided [`redact`] feeds one `(location, redaction)` pair
    /// at a time.
    ///
    /// [`redact`]: TextHandler::redact
    async fn redact_at(
        &mut self,
        location: &TextLocation,
        redaction: TextRedaction,
    ) -> Result<(), Error>;

    /// Apply every `(location, redaction)` pair in `redactions` to the
    /// handler in insertion order. The first error aborts the batch.
    ///
    /// The default loops [`redact_at`] in [`Redactions`] insertion
    /// order; handlers with ordering constraints (see
    /// [`AudioHandler::redact`]) override this default.
    ///
    /// [`redact_at`]: TextHandler::redact_at
    /// [`AudioHandler::redact`]: crate::handler::AudioHandler::redact
    async fn redact(
        &mut self,
        redactions: Redactions<TextLocation, TextRedaction>,
    ) -> Result<(), Error> {
        for (location, redaction) in redactions {
            self.redact_at(&location, redaction).await?;
        }
        Ok(())
    }
}
