//! Pipeline-level redaction trait.

use crate::ontology::entity::Entity;
use crate::ontology::policy::Policy;

/// Types that produce redaction decisions.
pub trait Redactable {
    /// The entities detected in this content.
    fn entities(&self) -> &[Entity];
    /// The policy governing redaction.
    fn policy(&self) -> Option<&Policy>;
}
