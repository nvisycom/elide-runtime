//! Format handler traits + supporting infrastructure.
//!
//! [`Handler`] is the base trait every format handler implements
//! (identify + encode). [`Handle<M>`] is the per-modality capability
//! trait: a format that exposes content for modality `M` implements
//! `Handle<M>`; multi-modality formats implement it once per modality.
//! [`Codable`] declares the codec-side wire types each modality
//! needs (`Data`, `Redaction`).
//!
//! Modality features control which `Handle<M>` impls and helpers are
//! compiled in. The default set (`text`, `tabular`) covers the
//! lightweight cases; opt into `image`, `audio`, or `rich` for the
//! heavier modalities that pull additional dependencies.

use nvisy_core::Error;
use nvisy_core::content::ContentData;
use nvisy_core::media::DocumentType;

#[cfg(feature = "audio")]
mod audio;
mod handle;
#[cfg(feature = "image")]
mod image;
mod policy;
mod redactions;
#[cfg(feature = "rich")]
mod rich;
#[cfg(feature = "tabular")]
mod tabular;
#[cfg(feature = "text")]
mod text;

use nvisy_core::content::ContentSource;
pub use nvisy_ontology::modality::Mergeable;

#[cfg(feature = "audio")]
pub use self::audio::*;
pub use self::handle::{Codable, Handle};
#[cfg(feature = "image")]
pub use self::image::*;
pub use self::policy::{ConflictPolicy, InsertError};
pub use self::redactions::Redactions;
#[cfg(feature = "rich")]
pub use self::rich::*;
#[cfg(feature = "tabular")]
pub use self::tabular::*;
#[cfg(feature = "text")]
pub use self::text::*;

/// Base trait implemented by all format handlers.
///
/// A handler holds loaded, validated content and provides methods to
/// identify and serialize it. Handlers are produced by their
/// corresponding [`Loader`].
///
/// Per-modality capability is provided by implementing [`Handle<M>`]
/// for each modality the handler exposes.
pub trait Handler: Send + Sync + 'static {
    /// The document type this handler represents.
    fn document_type(&self) -> DocumentType;

    /// Content source identity and lineage for this handler.
    fn source(&self) -> ContentSource;

    /// Serialize the current handler content back to [`ContentData`].
    fn encode(&self) -> Result<ContentData, Error>;
}

/// Trait implemented by format loaders.
///
/// A loader validates and parses raw content, producing the
/// corresponding handler.
#[async_trait::async_trait]
pub trait Loader: Send + Sync + 'static {
    /// The handler type this loader produces.
    type Handler: Handler;
    /// Strongly-typed parameters for loading.
    type Params: Send;

    /// Validate and parse the content, returning the loaded handler.
    async fn decode(
        &self,
        content: &ContentData,
        params: &Self::Params,
    ) -> Result<Self::Handler, Error>;
}
