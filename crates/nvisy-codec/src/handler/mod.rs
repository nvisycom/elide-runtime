//! Modality-keyed handler traits and supporting infrastructure.
//!
//! Each capability trait — [`TextHandler`], [`TabularHandler`],
//! [`ImageHandler`], [`AudioHandler`], [`RichHandler`] — extends the
//! base [`Handler`] trait with one modality's location streaming,
//! reading, and redaction surface. The concrete per-format
//! implementations live in `nvisy-formats`.
//!
//! Modality features control which traits and helpers are compiled
//! in. The default set (`text`, `tabular`) covers the lightweight
//! cases; opt into `image`, `audio`, or `rich` for the heavier
//! modalities that pull additional dependencies.

use nvisy_core::Error;
use nvisy_core::content::ContentData;
use nvisy_core::media::DocumentType;

#[cfg(feature = "audio")]
mod audio;
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
/// Capability-specific access is provided by the opt-in traits
/// per modality (e.g. [`TextHandler`], [`ImageHandler`]).
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
