//! Per-scan filters and hints: suppression, forced detection,
//! caller-supplied context, and the [`ScanContext`] that bundles
//! them.
//!
//! All types are part of the public API and configured by the
//! caller on each [`PatternEngine::scan_entities`] invocation.
//!
//! [`PatternEngine::scan_entities`]: super::PatternEngine::scan_entities

mod allow_list;
mod context_hint;
mod deny_list;
mod deny_scanner;

use serde::{Deserialize, Serialize};

pub use self::allow_list::AllowList;
pub use self::context_hint::ContextHint;
pub use self::deny_list::{DenyList, DenyRule};

/// Per-scan configuration for allow/deny lists and context hints.
///
/// Passed to [`PatternEngine::scan_entities`] to control
/// per-invocation suppression, forced detection, and context-aware
/// confidence boosting without rebuilding the engine.
///
/// All fields default to empty, so `ScanContext::default()` is a
/// no-op context. The type is `Serialize + Deserialize` so an HTTP
/// API can accept a `ScanContext` as JSON request body.
///
/// [`PatternEngine::scan_entities`]: super::PatternEngine::scan_entities
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ScanContext {
    /// Values to silently drop from results.
    #[serde(default)]
    pub allow: AllowList,
    /// Values to inject as synthetic matches when found in text.
    #[serde(default)]
    pub deny: DenyList,
    /// Caller-supplied context keywords, optionally scoped per
    /// [`EntityKind`]. The enhancer picks at most one bucket per
    /// match: the entry whose `kind == Some(match.entity_kind)`,
    /// or the first entry with `kind == None` as fallback.
    ///
    /// [`EntityKind`]: nvisy_ontology::entity::EntityKind
    #[serde(default)]
    pub hints: Vec<ContextHint>,
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{EntityCategory, EntityKind, RecognitionMethod};

    use super::*;

    #[test]
    fn scan_context_round_trips_via_json() {
        let mut deny = DenyList::new();
        deny.insert(
            "secret",
            DenyRule {
                category: EntityCategory::PersonalIdentity,
                entity_kind: EntityKind::PersonName,
                method: RecognitionMethod::regex("deny:secret"),
            },
        );
        let ctx = ScanContext {
            allow: AllowList::from_iter(["123-45-6789"]),
            deny,
            hints: vec![
                ContextHint {
                    kind: Some(EntityKind::GovernmentId),
                    keywords: vec!["social".into(), "ssn".into()],
                    window: Some(150),
                    boost: Some(0.25),
                },
                ContextHint {
                    kind: None,
                    keywords: vec!["medical record".into()],
                    ..ContextHint::default()
                },
            ],
        };

        let json = serde_json::to_string(&ctx).expect("serialize");
        let back: ScanContext = serde_json::from_str(&json).expect("deserialize");

        assert!(back.allow.contains("123-45-6789"));
        assert!(back.deny.contains("secret"));
        assert_eq!(back.hints.len(), 2);
        assert_eq!(back.hints[0].kind, Some(EntityKind::GovernmentId));
        assert_eq!(back.hints[0].keywords, vec!["social", "ssn"]);
        assert_eq!(back.hints[0].window, Some(150));
        assert_eq!(back.hints[0].boost, Some(0.25));
        assert_eq!(back.hints[1].kind, None);
        assert_eq!(back.hints[1].keywords, vec!["medical record"]);
    }

    #[test]
    fn empty_scan_context_deserializes_from_empty_object() {
        let ctx: ScanContext = serde_json::from_str("{}").expect("empty object");
        assert!(ctx.allow.is_empty());
        assert!(ctx.deny.is_empty());
        assert!(ctx.hints.is_empty());
    }
}
