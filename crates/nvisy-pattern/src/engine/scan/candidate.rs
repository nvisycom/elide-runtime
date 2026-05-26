//! [`EntityCandidate`]: internal output type from pattern scan phases.
//!
//! Lives in the [`scan`] module because it is the shared exchange
//! type between [`phases`] and the [`enhancer`]. Wraps a fully-built
//! [`Entity`] plus a per-pattern [`ContextRule`] used during the
//! context-aware enhancement pass; the rule is dropped before the
//! candidate becomes a final entity.
//!
//! [`scan`]: super
//! [`phases`]: super::phases
//! [`enhancer`]: super::enhancer

use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;

use crate::patterns::ContextRule;

/// A single candidate entity produced by the pattern engine's
/// internal scan phases.
///
/// Pairs a fully-built [`Entity<Text>`] with the per-pattern
/// [`ContextRule`] used by the context enhancer to apply
/// confidence boosts or penalties before the threshold filter
/// commits the entity to the final output.
#[derive(Debug, Clone)]
pub(crate) struct EntityCandidate {
    pub entity: Entity<Text>,
    pub context: Option<ContextRule>,
}

impl EntityCandidate {
    /// Pair an entity with its per-pattern context rule.
    pub(crate) fn new(entity: Entity<Text>, context: Option<ContextRule>) -> Self {
        Self { entity, context }
    }
}
