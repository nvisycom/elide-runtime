//! [`TextAt`]: resolve a per-modality location to its source text.
//!
//! The toolkit-side phase code (deduplication layers, validation
//! checks, redaction strategy bindings) all need to read back the
//! original text at a modality-typed location. The trait lives here
//! in core because every layer of the stack — toolkit components,
//! document phase drivers, custom user pipelines — bounds on it.
//!
//! Sibling to [`DataAt`]: where `DataAt` answers "what raw modality
//! payload lives at this location" (the typed `M::Data` envelope an
//! anonymizer rewrites), `TextAt` answers "what display text
//! represents this location" — the string a dedup layer or
//! validation check substring-scans. For text/tabular the two are
//! trivially related (the data *is* text). For image/audio they
//! diverge: `DataAt` reads raw pixels / samples through the codec,
//! while `TextAt` reads OCR'd / transcribed text from the document's
//! blocks.
//!
//! Concrete implementations live where the underlying resolver lives:
//! `nvisy-document` ships impls on `DocumentTree<M>` that consult the
//! codec handle for text/tabular and the document blocks for
//! image/audio.
//!
//! [`DataAt`]: super::DataAt

use crate::modality::Modality;

/// Resolve a modality-typed location to the corresponding source
/// text. Generic per-phase code bounds over `&impl TextAt<M>` and
/// dispatches uniformly across modalities.
#[async_trait::async_trait]
pub trait TextAt<M: Modality>: Sync {
    /// Resolve a location to its source-text representation, or
    /// `None` when no readable text exists at the location.
    async fn text_at(&self, location: &M::Location) -> Option<String>;
}
