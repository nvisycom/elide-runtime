//! [`Codable`] + [`Handle<M>`]: the unified codec surface.
//!
//! [`Codable`] is a codec-side extension of [`Modality`] declaring
//! the wire types a codec needs for a given modality — the per-
//! location `Data` payload (`TextData`/`ImageData`/`AudioData`) and
//! the per-location `Redaction` instruction
//! (`TextRedaction`/`ImageRedaction`/...). Implementing crates wire
//! one [`Codable`] impl per supported modality.
//!
//! [`Handle<M>`] is the per-modality capability trait every format
//! handler implements: stream locations, read a location's data,
//! apply a redaction at a location, and (optionally) batch a set of
//! redactions in one pass. A format that supports multiple
//! modalities (e.g. PDF for text + image) implements `Handle<M>`
//! once per modality.
//!
//! [`Modality`]: nvisy_ontology::modality::Modality

use nvisy_core::Error;
use nvisy_ontology::modality::Modality;

use super::{LocationStream, Redactions};
use crate::handler::Handler;

/// Codec-side associated types for a modality: the wire payloads a
/// format handler reads/writes for that modality.
pub trait Codable: Modality {
    /// Per-location data payload returned by [`Handle::read`].
    type Data: Send + 'static;
    /// Per-location redaction instruction applied by [`Handle::redact_at`].
    type Redaction: Send + Sync + 'static;
}

/// Per-modality capability trait for format handlers.
///
/// A handler that exposes content for modality `M` implements
/// `Handle<M>`. Multi-modality formats (e.g. PDF) implement
/// `Handle<Text>` and `Handle<Image>` on the same struct.
///
/// # Location semantics
///
/// `M`-typed locations are coordinates in the handler's serialized
/// form, but what that means varies by modality:
///
/// - **Text** — byte offsets into the encoded bytes (including JSON
///   quoting, CSV delimiters, etc.). Use [`read`] to extract the
///   logical value rather than slicing the serialized bytes
///   directly.
/// - **Image** — pixel-space bounding boxes against the decoded
///   image, qualified by `image_id` and `page_number`.
/// - **Audio** — microsecond time spans into the source stream,
///   qualified by `audio_id` and `speaker_id`.
/// - **Tabular** — cell coordinates (`row_index`, `column_index`,
///   optional `sheet_name`) with optional intra-cell byte offsets.
///
/// [`read`]: Handle::read
#[async_trait::async_trait]
pub trait Handle<M: Codable>: Handler {
    /// Stream of locations exposed by this handler for modality `M`,
    /// each tagged with the handler's [`ContentSource`].
    ///
    /// [`ContentSource`]: nvisy_core::content::ContentSource
    fn locations(&self) -> LocationStream<'_, M>;

    /// Read the per-modality payload at the given location.
    ///
    /// Returns `None` if the location is out of bounds.
    async fn read(&self, location: &M) -> Option<M::Data>;

    /// Apply a single redaction at the given location, mutating in
    /// place. Implementations need not handle iteration or overlap —
    /// the default [`redact`] feeds one `(location, redaction)` pair
    /// at a time in insertion order.
    ///
    /// [`redact`]: Handle::redact
    async fn redact_at(&mut self, location: &M, redaction: M::Redaction) -> Result<(), Error>;

    /// Apply every `(location, redaction)` pair in `redactions` to
    /// the handler in insertion order. The first error aborts the
    /// batch.
    ///
    /// Handlers with ordering constraints (e.g. audio time-span
    /// merging) override this default with their own batched logic.
    async fn redact(&mut self, redactions: Redactions<M, M::Redaction>) -> Result<(), Error> {
        for (location, redaction) in redactions.items {
            self.redact_at(&location, redaction).await?;
        }
        Ok(())
    }
}
