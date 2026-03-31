//! Per-document state accumulated across pipeline operations.
//!
//! A [`DocumentEnvelope`] is created at import and travels through
//! every operation in the pipeline. Each stage reads from and writes to
//! the envelope, progressively enriching it with entities and audit
//! records until the document is fully redacted.
//!
//! ```text
//! ContentData
//!   ↓ Import
//! DocumentEnvelope { document, … }
//!   ↓ OCR / NER / CV / PatternMatch
//! DocumentEnvelope { document, entities, … }
//!   ↓ Deduplication / Ensemble
//! DocumentEnvelope { document, entities (merged), … }
//!   ↓ PolicyEvaluation
//! DocumentEnvelope { document, entities, audit { decisions, records }, … }
//!   ↓ Redaction
//! DocumentEnvelope { document (redacted), entities, audit { … } }
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
use nvisy_ontology::entity::Entities;
use nvisy_ontology::provenance::Audit;

use super::context::SharedData;

/// Per-document state that flows through the entire pipeline.
///
/// Created by import from a decoded [`Document`], then progressively
/// enriched by detection, policy, and redaction operations. Operations
/// receive `&mut DocumentEnvelope` and access run-wide shared state
/// via the [`shared`](DocumentEnvelope::shared) field.
pub struct DocumentEnvelope {
    /// The decoded document content (text, image, audio, or rich).
    ///
    /// Modified in-place during the redaction stage.
    pub document: Document,

    /// Content metadata (MIME type, filename, etc.) from the original upload.
    ///
    /// Preserved through the pipeline so operations can access the
    /// original filename, MIME type, and other descriptive attributes.
    pub metadata: ContentMetadata,

    /// Entities detected by inference and processing operations.
    ///
    /// Populated by OCR, NER, computer vision, pattern matching,
    /// and manual annotation. Refined by deduplication and ensemble
    /// fusion before policy evaluation.
    pub entities: Entities,

    /// Reference-data contexts loaded by [`LoadContext`] nodes.
    ///
    /// Populated as envelopes pass through `LoadContext` nodes and
    /// available to downstream operations that need contextual data.
    ///
    /// [`LoadContext`]: nvisy_ontology::workflow::LoadContext
    pub contexts: Contexts,

    /// Per-document audit trail: execution log, redaction decisions,
    /// and redaction records.
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
            entities: Entities::new(),
            contexts: Contexts::new(),
            audit,
            shared,
        }
    }

    /// Number of detected entities.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }
}

impl std::fmt::Debug for DocumentEnvelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentEnvelope")
            .field("document_type", &self.document.document_type())
            .field("source", &self.document.source())
            .field("entities", &self.entities.len())
            .field("contexts", &self.contexts.len())
            .field("audit_entries", &self.audit.len())
            .field("decisions", &self.audit.decisions.len())
            .field("records", &self.audit.records.len())
            .finish()
    }
}
