//! [`ScanContext`]: per-scan allow/deny list configuration.

use super::allow_list::AllowList;
use super::deny_list::DenyList;

/// Per-scan configuration for allow and deny lists.
///
/// Passed to [`PatternEngine::scan_text`](super::PatternEngine::scan_text)
/// to control per-invocation suppression and forced detection without
/// rebuilding the engine.
///
/// # Examples
///
/// ```rust,ignore
/// use nvisy_pattern::prelude::*;
/// use nvisy_ontology::entity::{EntityCategory, EntityKind, RecognitionMethod};
///
/// let ctx = ScanContext::new()
///     .with_allow(AllowList::new().with("000-00-0000"))
///     .with_deny(DenyList::new().with("secret", DenyRule {
///         category: EntityCategory::PersonalIdentity,
///         entity_kind: EntityKind::PersonName,
///         method: RecognitionMethod::annotation(Some("test".into())),
///     }));
/// let matches = PatternEngine::instance().scan_text("text", &ctx);
/// ```
#[derive(Debug, Clone, Default)]
pub struct ScanContext {
    pub(super) allow: AllowList,
    pub(super) deny: DenyList,
}

impl ScanContext {
    /// Create an empty scan context (no allow/deny filtering).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the allow list.
    pub fn with_allow(mut self, list: AllowList) -> Self {
        self.allow = list;
        self
    }

    /// Set the deny list.
    pub fn with_deny(mut self, list: DenyList) -> Self {
        self.deny = list;
        self
    }
}
