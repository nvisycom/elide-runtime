//! [`PatternEngine`]: pre-compiled scanning engine.

use std::fmt;
use std::sync::LazyLock;

use nvisy_ontology::entity::Entity;
use regex::RegexSet;

use super::builder::PatternEngineBuilder;
use super::filter::ScanContext;
use super::scan::dedup::{dedup_overlapping, sort_for_dedup};
use super::scan::entries::{DictEntry, RegexEntry};
use super::scan::pattern_match::RawMatch;
use super::scan::phases::{scan_deny_list, scan_dict, scan_regex};
use crate::validators::ValidatorResolver;

const TARGET: &str = "nvisy_pattern::engine";

/// Pre-compiled engine that scans text against all registered patterns.
///
/// Scanning runs in three phases:
///
/// 1. **Regex**: a [`RegexSet`] pre-filter selects candidate patterns,
///    then each matching regex extracts offsets and values.
/// 2. **Dictionary**: Aho-Corasick automata perform literal multi-pattern
///    matching against known-value dictionaries.
/// 3. **Deny list**: known sensitive values not already matched are
///    injected as synthetic matches with confidence `1.0`.
///
/// Allow-list filtering is applied inline during phases 1 and 2.
///
/// Build via [`PatternEngine::builder`] or use [`PatternEngine::instance`]
/// for the singleton with all built-in patterns.
pub struct PatternEngine {
    pub(super) regex_set: RegexSet,
    pub(super) regex_entries: Vec<RegexEntry>,
    pub(super) dict_entries: Vec<DictEntry>,
    pub(super) validators: ValidatorResolver,
    pub(super) confidence_threshold: f64,
}

impl fmt::Debug for PatternEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PatternEngine")
            .field("regex_patterns", &self.regex_entries.len())
            .field("dict_patterns", &self.dict_entries.len())
            .field("confidence_threshold", &self.confidence_threshold)
            .finish()
    }
}

impl PatternEngine {
    /// Return a reference to the lazily-initialised default engine
    /// containing all built-in patterns.
    pub fn instance() -> &'static Self {
        &DEFAULT_ENGINE
    }

    /// Create a new [`PatternEngineBuilder`].
    pub fn builder() -> PatternEngineBuilder {
        PatternEngineBuilder::default()
    }

    /// Scan `text` and return detected entities with [`TextLocation`]s.
    ///
    /// Matches whose value appears in the allow list are suppressed.
    /// Deny-list values found in the text are injected as synthetic
    /// matches with confidence `1.0` when not already matched.
    ///
    /// Each entity carries a [`TextLocation`] with `start_offset` and
    /// `end_offset` set from the match.
    ///
    /// [`TextLocation`]: nvisy_ontology::entity::TextLocation
    #[tracing::instrument(level = "trace", target = TARGET, skip(self, text, ctx), fields(text_len = text.len(), entities = tracing::field::Empty))]
    pub fn scan_entities(&self, text: &str, ctx: &ScanContext) -> Vec<Entity> {
        let mut raw = self.scan_raw(text, ctx);

        // Apply context-based confidence adjustment before threshold
        // filtering so a match just below threshold can be promoted by
        // keyword co-occurrence.
        for m in &mut raw {
            m.apply_context_adjustment(text);
        }

        sort_for_dedup(&mut raw);
        let deduped = dedup_overlapping(&raw);

        let threshold = self.confidence_threshold;
        let entities: Vec<Entity> = deduped
            .into_iter()
            .filter(|m| m.confidence >= threshold)
            .map(|m| {
                tracing::trace!(
                    target: TARGET,
                    pattern = m.pattern_name.as_deref().unwrap_or("deny_list"),
                    kind = %m.entity_kind,
                    confidence = m.confidence,
                    start = m.start,
                    end = m.end,
                    "matched entity",
                );

                m.into_entity()
            })
            .collect();

        tracing::Span::current().record("entities", entities.len());
        entities
    }

    /// Internal: scan and return raw matches.
    pub(super) fn scan_raw(&self, text: &str, ctx: &ScanContext) -> Vec<RawMatch> {
        let mut results = Vec::new();

        let candidates = self.regex_set.matches(text).into_iter();
        scan_regex(
            candidates,
            &self.regex_entries,
            &self.validators,
            text,
            &ctx.allow,
            &mut results,
        );
        scan_dict(&self.dict_entries, text, &ctx.allow, &mut results);
        scan_deny_list(text, &ctx.deny, &mut results);

        results
    }
}

static DEFAULT_ENGINE: LazyLock<PatternEngine> = LazyLock::new(|| {
    PatternEngine::builder()
        .build()
        .expect("built-in patterns must compile")
});
