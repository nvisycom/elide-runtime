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
//!   ↓ Policy Evaluation + Redaction
//! DocumentEnvelope { document (redacted), audit { entities, entries, … } }
//! ```
//!
//! Each operation receives `&mut DocumentEnvelope` and reads/writes
//! fields directly. Run-wide shared state (policies, registry, key
//! provider) is available via the [`shared`](DocumentEnvelope::shared)
//! field.

use std::sync::Arc;

use nvisy_codec::ContentHandle;
use nvisy_core::content::ContentMetadata;
use nvisy_ontology::context::Contexts;
use nvisy_ontology::entity::{Annotations, Entity};
use nvisy_ontology::provenance::{Audit, RedactionMap};

mod document;
mod shared;

pub use self::document::Document;
pub use self::shared::SharedData;

/// Per-document state that flows through the entire pipeline.
///
/// Created by import from a decoded [`ContentHandle`], then progressively
/// enriched by detection, policy, and redaction operations. Operations
/// receive `&mut DocumentEnvelope` and access run-wide shared state
/// via the [`shared`](DocumentEnvelope::shared) field.
///
/// Detected entities live on [`audit.entities`](Audit::entities),
/// not as a top-level field.
pub struct DocumentEnvelope {
    /// The document: content handle + metadata + artifacts.
    ///
    /// The content handle is modified in-place during the redaction stage.
    pub document: Document,

    /// User-supplied annotations (inclusions, exclusions, labels)
    /// attached at upload time. Set during import from content metadata.
    pub(crate) annotations: Annotations,

    /// Reference-data contexts loaded by [`LoadContext`] nodes.
    ///
    /// [`LoadContext`]: nvisy_ontology::workflow::LoadContext
    pub contexts: Contexts,

    /// Per-document audit trail: entities, processing log, and
    /// redaction records.
    pub audit: Audit,

    /// Mapping of entity IDs to original and replacement values.
    /// Populated during redaction (phase 4). Not included in the
    /// public audit response, stored separately under access control.
    pub redaction_map: RedactionMap,

    /// Run-wide shared state (policies, registry, key provider, etc.).
    ///
    /// Cheaply cloneable (`Arc`): all envelopes in a run share the
    /// same underlying data.
    pub shared: Arc<SharedData>,
}

impl DocumentEnvelope {
    /// Create a new envelope from a content handle and metadata.
    pub fn new(handle: ContentHandle, metadata: ContentMetadata, shared: Arc<SharedData>) -> Self {
        let document = Document::new(handle, metadata);
        let audit = Audit::new(document.source());
        Self {
            document,
            annotations: Annotations::new(),
            contexts: Contexts::new(),
            audit,
            redaction_map: RedactionMap::new(),
            shared,
        }
    }

    /// Number of detected entities.
    pub fn entity_count(&self) -> usize {
        self.audit.entities.len()
    }

    /// Add detected entities, assigning sensitivity from entity kind
    /// and filtering out any that fall within exclusion annotations.
    pub async fn add_entities(&mut self, entities: impl IntoIterator<Item = Entity>) {
        for mut entity in entities {
            // Assign sensitivity if not already set.
            if entity.sensitivity.is_none() {
                entity.sensitivity = Some(entity.entity_kind.sensitivity());
            }
            if self.annotations.is_empty() {
                self.audit.entities.push(entity);
            } else {
                let value = self.document.value_at(&entity.location).await;
                if !self.annotations.is_excluded(&entity, value.as_deref()) {
                    self.audit.entities.push(entity);
                }
            }
        }
    }
}

impl std::fmt::Debug for DocumentEnvelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentEnvelope")
            .field("document_type", &self.document.document_type())
            .field("source", &self.document.source())
            .field("entities", &self.audit.entities.len())
            .field("contexts", &self.contexts.len())
            .field("entries", &self.audit.entries.len())
            .finish()
    }
}
