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
//!   ↓ OCR / NER / CV / PatternMatch  →  DetectedEntities
//! DocumentEnvelope { document, entities, … }
//!   ↓ Deduplication / Ensemble        →  RefinedEntities
//! DocumentEnvelope { document, entities (merged), … }
//!   ↓ PolicyEvaluation                →  PolicyOutcome
//! DocumentEnvelope { document, entities, audit { decisions, records }, … }
//!   ↓ Redaction
//! DocumentEnvelope { document (redacted), entities, audit { … } }
//! ```
//!
//! Operations produce typed patch values that implement [`ApplyPatch`].
//! The orchestrator merges each patch via [`DocumentEnvelope::apply`].

mod apply;
mod audit;
mod detection;
mod policy;

use nvisy_codec::Document;
use nvisy_ontology::context::Contexts;
use nvisy_ontology::entity::Entities;

pub use self::apply::ApplyPatch;
pub use self::audit::OperationEntry;
pub use self::detection::{DetectedEntities, RefinedEntities};
pub use self::policy::PolicyOutcome;
use crate::provenance::Audit;

/// Per-document state that flows through the entire pipeline.
///
/// Created by import from a decoded [`Document`], then progressively
/// enriched by detection, policy, and redaction operations. The
/// orchestrator passes the envelope (wrapped in a `ParallelContext`
/// or `SequentialContext`) between stages.
pub struct DocumentEnvelope {
    /// The decoded document content (text, image, audio, or rich).
    ///
    /// Modified in-place during the redaction stage.
    pub document: Document,

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
    /// [`LoadContext`]: crate::graph::LoadContext
    pub contexts: Contexts,

    /// Per-document audit trail: execution log, redaction decisions,
    /// and redaction records.
    pub audit: Audit,
}

impl DocumentEnvelope {
    /// Create a new envelope from a freshly decoded document.
    pub fn new(document: Document) -> Self {
        let audit = Audit::new(document.source());
        Self {
            document,
            entities: Entities::new(),
            contexts: Contexts::new(),
            audit,
        }
    }

    /// Number of detected entities.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Merge an operation's output into this envelope.
    pub fn apply(&mut self, patch: impl ApplyPatch) {
        patch.apply(self);
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
