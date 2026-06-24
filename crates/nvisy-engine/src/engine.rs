//! [`EngineHandle`]: the single dependency entry point every
//! orchestrator call takes.
//!
//! Bundles the two long-lived runtime registries:
//!
//! - The [`RegistryHandle`] over [`fjall`] (policies, contexts,
//!   runs).
//! - The [`FormatRegistry`] over elide's codec set (decode bytes
//!   → modality-typed handle at analyze + apply time).
//!
//! Cheaply cloneable — both fields are `Arc`-backed under the
//! hood — so the server hands a clone to every HTTP handler
//! without coordinating lifetime.
//!
//! [`FormatRegistry`]: elide::codec::FormatRegistry
//! [`fjall`]: ::fjall

use std::path::Path;
use std::sync::Arc;

use elide::codec::FormatRegistry;
use nvisy_core::Result;

use crate::registry::RegistryHandle;

/// Cheaply-cloneable bundle of the persistence registry + codec
/// registry every orchestrator entry point consumes.
#[derive(Clone)]
pub struct EngineHandle {
    registry: RegistryHandle,
    formats: Arc<FormatRegistry>,
}

impl EngineHandle {
    /// Open (or create) the engine database at `path` and pair it
    /// with elide's built-in codec set
    /// ([`FormatRegistry::with_builtin`]).
    pub fn open(path: &Path) -> Result<Self> {
        let registry = RegistryHandle::open(path)?;
        let formats = Arc::new(FormatRegistry::with_builtin());
        Ok(Self { registry, formats })
    }

    /// Open (or create) the engine database at `path` and pair it
    /// with a caller-supplied `formats` registry. Useful for tests
    /// that need to register fake codecs, or for deployments that
    /// extend the built-in set.
    pub fn with_formats(path: &Path, formats: FormatRegistry) -> Result<Self> {
        let registry = RegistryHandle::open(path)?;
        Ok(Self {
            registry,
            formats: Arc::new(formats),
        })
    }

    /// The persistence registry. Holds the fjall keyspaces every
    /// resource module reads and writes.
    pub fn registry(&self) -> &RegistryHandle {
        &self.registry
    }

    /// The codec registry. Pipeline calls reach for it to decode
    /// raw bytes into a modality-typed
    /// [`DocumentHandle`](elide::codec::DocumentHandle).
    pub fn formats(&self) -> &FormatRegistry {
        &self.formats
    }

    /// Flush pending writes to disk. The server's HTTP layer
    /// calls this on graceful shutdown.
    pub fn sync(&self) -> Result<()> {
        self.registry.sync()
    }
}
