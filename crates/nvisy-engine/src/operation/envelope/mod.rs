//! Per-document state accumulated across pipeline operations.
//!
//! A [`DocumentEnvelope`] is created at import and travels through
//! every operation in the pipeline. Each stage reads from and writes to
//! the envelope until the document is fully redacted.
//!
//! ```text
//! ContentData
//!   ↓ Import
//! DocumentEnvelope { document, audit, … }
//!   ↓ OCR / NER / CV / PatternMatch
//! DocumentEnvelope { document, audit { entities, … } }
//!   ↓ Deduplication / Ensemble
//! DocumentEnvelope { document, audit { entities (merged), … } }
//!   ↓ PolicyEvaluation
//! DocumentEnvelope { document, audit { entities, records, … } }
//!   ↓ Redaction
//! DocumentEnvelope { document (redacted), audit { … } }
//! ```
//!
//! Each operation receives `&mut DocumentEnvelope` and reads/writes
//! fields directly. Run-wide shared state (policies, registry, key
//! provider) is available via the [`shared`](DocumentEnvelope::shared)
//! field.

use std::sync::Arc;

use nvisy_codec::Document;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::context::Contexts;
use nvisy_ontology::provenance::Audit;

mod shared;

pub use self::shared::SharedData;

/// Per-document state that flows through the entire pipeline.
///
/// Created by import from a decoded [`Document`], then progressively
/// enriched by detection, policy, and redaction operations. Operations
/// receive `&mut DocumentEnvelope` and access run-wide shared state
/// via the [`shared`](DocumentEnvelope::shared) field.
///
/// Detected entities live on [`audit.entities`](Audit::entities),
/// not as a top-level field.
pub struct DocumentEnvelope {
    /// The decoded document content (text, image, audio, or rich).
    ///
    /// Modified in-place during the redaction stage.
    pub document: Document,

    /// Content metadata (MIME type, filename, etc.) from the original upload.
    pub metadata: ContentMetadata,

    /// Reference-data contexts loaded by [`LoadContext`] nodes.
    ///
    /// [`LoadContext`]: nvisy_ontology::workflow::LoadContext
    pub contexts: Contexts,

    /// Per-document audit trail: entities, processing log, and
    /// redaction records.
    pub audit: Audit,

    /// Run-wide shared state (policies, registry, key provider, etc.).
    ///
    /// Cheaply cloneable (`Arc`): all envelopes in a run share the
    /// same underlying data.
    pub shared: Arc<SharedData>,
}

impl DocumentEnvelope {
    /// Create a new envelope from a freshly decoded document.
    pub fn new(document: Document, metadata: ContentMetadata, shared: Arc<SharedData>) -> Self {
        let audit = Audit::new(document.source());
        Self {
            document,
            metadata,
            contexts: Contexts::new(),
            audit,
            shared,
        }
    }

    /// Number of detected entities.
    pub fn entity_count(&self) -> usize {
        self.audit.entities.len()
    }
}

impl std::fmt::Debug for DocumentEnvelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentEnvelope")
            .field("document_type", &self.document.document_type())
            .field("source", &self.document.source())
            .field("entities", &self.audit.entities.len())
            .field("contexts", &self.contexts.len())
            .field("audit_entries", &self.audit.len())
            .field("records", &self.audit.records.len())
            .finish()
    }
}
