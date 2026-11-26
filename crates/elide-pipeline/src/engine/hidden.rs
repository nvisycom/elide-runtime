//! Hidden content: [`Engine::hidden_content`], the preview a
//! reviewer decides from.

use elide::Result;
use elide_review::HiddenItem;
use elide_schema::policy::Policy;

use super::Engine;
use crate::audit::Audit;

impl Engine {
    /// Every hidden-content item in `audit`, and what anonymize
    /// would do with it under `policies`.
    ///
    /// Analyze makes each metadata field and collaboration item
    /// (comment, comment author, tracked change) an entity. An item
    /// is removed when the policies' catalog declares its label,
    /// when a recognizer found something in its value, or when a
    /// reviewer removed it; every other item is kept whole. This is
    /// the plan anonymize applies, worked out the same way, so what
    /// a reviewer is shown is what happens.
    ///
    /// A reviewer changes it through
    /// [`EditSet::hidden`](crate::entity::EditSet::hidden): remove
    /// an item, keep one, or remove all. Call this again after
    /// applying those edits to see the result.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](elide::ErrorKind::Configuration) if
    /// `policies` cannot compile a catalog, as analyze would.
    pub fn hidden_content(&self, audit: &Audit, policies: &[Policy]) -> Result<Vec<HiddenItem>> {
        Ok(self
            .provider()
            .hidden_content(&audit.report, &audit.recognition, policies)?
            .into_items())
    }
}
