//! Entity detection patches.

use nvisy_ontology::entity::Entities;

use super::apply::ApplyPatch;
use super::DocumentEnvelope;

/// New entities discovered by a detection operation (NER, OCR, CV,
/// pattern match, manual annotation).
///
/// Appended to the envelope's existing entity set.
pub struct DetectedEntities(pub Entities);

impl ApplyPatch for DetectedEntities {
    fn apply(self, envelope: &mut DocumentEnvelope) {
        envelope.entities.extend(self.0);
    }
}

/// A fully recomputed entity set produced by refinement operations
/// (deduplication, ensemble fusion).
///
/// Replaces the envelope's entity set entirely.
pub struct RefinedEntities(pub Entities);

impl ApplyPatch for RefinedEntities {
    fn apply(self, envelope: &mut DocumentEnvelope) {
        envelope.entities = self.0;
    }
}
