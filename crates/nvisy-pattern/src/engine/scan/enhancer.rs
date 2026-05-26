//! [`ContextEnhancer`]: post-scan confidence adjustment using each
//! candidate's per-pattern [`ContextRule`] plus caller-supplied
//! [`ContextHint`]s.
//!
//! Lives next to [`EntityCandidate`] because it operates on a slice
//! of candidates before the threshold filter. Kept crate-private;
//! the orchestration goes through [`PatternEngine::scan_entities`].
//!
//! [`EntityCandidate`]: super::candidate::EntityCandidate
//!
//! # Semantics
//!
//! Strict-Presidio: a match whose pattern declares no `ContextRule`
//! is left alone, regardless of any [`ContextHint`]s. The pattern
//! must opt into context-aware scoring; hints layer on top of
//! opt-in.
//!
//! Per-match bucket resolution: the enhancer picks at most one
//! [`ContextHint`] per match — the entry whose `kind ==
//! Some(match.entity_kind)`, falling back to the first entry with
//! `kind == None` (the global bucket) if there is no kind-specific
//! match. This prevents, for example, a `"DOB"` hint meant for a
//! date pattern from boosting a context-aware SSN pattern.
//!
//! When a [`ContextHint`] applies:
//!
//! - The keyword set searched in the window is `rule.keywords ∪
//!   hint.keywords`.
//! - The window radius is `hint.window` when both `hint.window` is
//!   `Some` and `hint.keywords` is non-empty; otherwise
//!   `rule.window`.
//! - The boost applied on a keyword hit is `hint.boost` when both
//!   `hint.boost` is `Some` and `hint.keywords` is non-empty;
//!   otherwise `rule.boost`.
//! - `rule.penalty` is unaffected by hints — it applies whenever no
//!   keyword (from either source) is found and the rule's own
//!   penalty is positive.
//!
//! [`ContextRule`]: crate::patterns::ContextRule
//! [`ContextHint`]: super::super::filter::ContextHint
//! [`PatternEngine::scan_entities`]: super::super::PatternEngine::scan_entities

use nvisy_ontology::entity::RecognitionMethod;

use super::candidate::EntityCandidate;
use crate::engine::filter::ContextHint;

/// Apply context-aware confidence adjustments to a slice of
/// candidates.
pub(crate) struct ContextEnhancer<'a> {
    text: &'a str,
    hints: &'a [ContextHint],
}

impl<'a> ContextEnhancer<'a> {
    /// Construct an enhancer scoped to `text` and the caller's
    /// `hints`.
    pub(crate) fn new(text: &'a str, hints: &'a [ContextHint]) -> Self {
        Self { text, hints }
    }

    /// Apply the enhancement pass in place.
    pub(crate) fn enhance(&self, candidates: &mut [EntityCandidate]) {
        for c in candidates {
            self.enhance_one(c);
        }
    }

    /// Find the most-specific hint bucket for this candidate: prefer
    /// a kind-specific entry, fall back to the first global entry
    /// (`kind == None`), or `None` if neither exists.
    fn select_hint(&self, c: &EntityCandidate) -> Option<&ContextHint> {
        let kind = c.entity.entity_kind;
        let kind_specific = self.hints.iter().find(|h| h.kind == Some(kind));
        if kind_specific.is_some() {
            return kind_specific;
        }
        self.hints.iter().find(|h| h.kind.is_none())
    }

    fn enhance_one(&self, c: &mut EntityCandidate) {
        // Strict-Presidio: patterns without a ContextRule opt out.
        let rule = match &c.context {
            Some(r) => r,
            None => return,
        };

        // Pick at most one applicable hint bucket. None still means
        // "apply the rule's own behavior."
        let hint = self.select_hint(c);

        // Per-call overrides only kick in when the selected hint
        // actually supplied keywords. Empty keywords + Some window
        // or boost would be the caller silently retuning
        // context-aware matches without contributing information.
        let hint_active = hint.map(|h| !h.is_empty()).unwrap_or(false);
        let window = if hint_active {
            hint.and_then(|h| h.window).unwrap_or(rule.window)
        } else {
            rule.window
        };
        let boost = if hint_active {
            hint.and_then(|h| h.boost).unwrap_or(rule.boost)
        } else {
            rule.boost
        };

        let span = &c.entity.location;
        let search_start = walk_chars_back(self.text, span.start_offset, window);
        let search_end = walk_chars_forward(self.text, span.end_offset, window);
        let window_text = &self.text[search_start..search_end];

        // Search the rule's own keywords plus the selected hint's
        // keywords (when active) in the same window. Build the
        // iterator inside each branch because Iterator::any
        // consumes self and we want to keep both branches
        // syntactically symmetric.
        let extras: &[String] = if hint_active {
            hint.map(|h| h.keywords.as_slice()).unwrap_or(&[])
        } else {
            &[]
        };

        let found = if rule.case_sensitive {
            rule.keywords
                .iter()
                .chain(extras.iter())
                .any(|kw| window_text.contains(kw.as_str()))
        } else {
            let lower = window_text.to_lowercase();
            rule.keywords
                .iter()
                .chain(extras.iter())
                .any(|kw| lower.contains(&kw.to_lowercase()))
        };

        let adjusted = if found {
            c.entity.confidence = c.entity.confidence.saturating_add(boost);
            true
        } else if rule.penalty > 0.0 {
            c.entity.confidence = c.entity.confidence.saturating_sub(rule.penalty);
            true
        } else {
            false
        };

        if adjusted {
            for method in &mut c.entity.recognition_methods {
                if let RecognitionMethod::Pattern(p) = method {
                    p.contextual = true;
                }
            }
        }
    }
}

/// Walk backward from `byte_anchor` by at most `chars` Unicode
/// scalar values, returning the resulting byte offset. UTF-8 safe.
fn walk_chars_back(text: &str, byte_anchor: usize, chars: usize) -> usize {
    let anchor = byte_anchor.min(text.len());
    text[..anchor]
        .char_indices()
        .rev()
        .nth(chars.saturating_sub(1))
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

/// Walk forward from `byte_anchor` by at most `chars` Unicode
/// scalar values, returning the resulting byte offset. UTF-8 safe.
fn walk_chars_forward(text: &str, byte_anchor: usize, chars: usize) -> usize {
    let anchor = byte_anchor.min(text.len());
    // `nth(chars)` is the position *after* `chars` characters; if the
    // tail has fewer characters than that we return the end of `text`.
    let tail = &text[anchor..];
    match tail.char_indices().nth(chars) {
        Some((idx, _)) => anchor + idx,
        None => text.len(),
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::{Entity, EntityCategory, EntityKind, RecognitionMethod};
    use nvisy_ontology::modality::Text;
    use nvisy_ontology::primitive::Confidence;

    use super::*;
    use crate::patterns::ContextRule;

    fn rule(keywords: &[&str], window: usize, boost: f64, penalty: f64) -> ContextRule {
        ContextRule {
            keywords: keywords.iter().map(|s| (*s).to_owned()).collect(),
            window,
            boost,
            penalty,
            case_sensitive: false,
        }
    }

    fn match_with(
        rule: Option<ContextRule>,
        kind: EntityKind,
        start: usize,
        end: usize,
        conf: f64,
    ) -> EntityCandidate {
        EntityCandidate::new(
            Entity::builder()
                .with_category(EntityCategory::PersonalIdentity)
                .with_entity_kind(kind)
                .with_recognition_methods(vec![RecognitionMethod::regex("test")])
                .with_confidence(Confidence::clamped(conf))
                .with_location(Text::new(start, end))
                .build()
                .expect("required fields provided"),
            rule,
        )
    }

    fn ssn_match(
        rule: Option<ContextRule>,
        start: usize,
        end: usize,
        conf: f64,
    ) -> EntityCandidate {
        match_with(rule, EntityKind::GovernmentId, start, end, conf)
    }

    #[test]
    fn no_rule_is_left_alone_even_with_hints() {
        let hints = vec![ContextHint {
            kind: Some(EntityKind::GovernmentId),
            keywords: vec!["foo".into()],
            window: Some(500),
            boost: Some(0.9),
        }];
        let mut matches = vec![ssn_match(None, 10, 20, 0.5)];
        let text = "foo lorem ipsum foo dolor sit foo";
        ContextEnhancer::new(text, &hints).enhance(&mut matches);
        assert_eq!(
            matches[0].entity.confidence.get(),
            0.5,
            "no rule = no change"
        );
    }

    #[test]
    fn rule_keywords_alone_still_boost_without_hints() {
        let r = rule(&["ssn"], 20, 0.1, 0.0);
        let text = "the ssn is 123-45-6789 right here.";
        let mut matches = vec![ssn_match(Some(r), 11, 22, 0.5)];
        ContextEnhancer::new(text, &[]).enhance(&mut matches);
        assert!(
            (matches[0].entity.confidence.get() - 0.6).abs() < 1e-9,
            "boost should apply: got {}",
            matches[0].entity.confidence.get(),
        );
    }

    #[test]
    fn hint_keywords_merge_with_rule_keywords_for_matching_kind() {
        let r = rule(&["ssn"], 40, 0.1, 0.0);
        let text = "social security number is right here: 123";
        let mut matches = vec![ssn_match(Some(r), 38, 41, 0.5)];
        let hints = vec![ContextHint {
            kind: Some(EntityKind::GovernmentId),
            keywords: vec!["social".into()],
            ..ContextHint::default()
        }];
        ContextEnhancer::new(text, &hints).enhance(&mut matches);
        assert!(
            matches[0].entity.confidence.get() > 0.5,
            "hint keyword should trigger boost: got {}",
            matches[0].entity.confidence.get(),
        );
    }

    #[test]
    fn kind_targeted_hint_does_not_apply_to_other_kinds() {
        // Hint targets DateOfBirth; match is GovernmentId. Boost
        // should NOT apply even though the hint keyword "dob" is
        // present in the window.
        let r = rule(&["ssn"], 40, 0.1, 0.0);
        let text = "DOB and SSN both 123-45-6789 here";
        let mut matches = vec![ssn_match(Some(r), 17, 28, 0.5)];
        let hints = vec![ContextHint {
            kind: Some(EntityKind::DateOfBirth),
            keywords: vec!["dob".into()],
            ..ContextHint::default()
        }];
        ContextEnhancer::new(text, &hints).enhance(&mut matches);
        // "ssn" is in the rule and IS in the window -> the rule's
        // own keywords still trigger the boost; the dob hint
        // contributed nothing. Confidence rises by rule.boost only.
        assert!(
            (matches[0].entity.confidence.get() - 0.6).abs() < 1e-9,
            "only rule.boost should apply (dob hint targets wrong kind): got {}",
            matches[0].entity.confidence.get(),
        );
    }

    #[test]
    fn global_fallback_hint_applies_when_no_kind_specific() {
        // No kind-specific hint for GovernmentId; the global
        // fallback (kind=None) applies.
        let r = rule(&[], 40, 0.1, 0.0); // empty rule keywords, only hint can trigger
        let text = "medical record number is 123 right here";
        let mut matches = vec![ssn_match(Some(r), 25, 28, 0.5)];
        let hints = vec![ContextHint {
            kind: None,
            keywords: vec!["medical".into()],
            ..ContextHint::default()
        }];
        ContextEnhancer::new(text, &hints).enhance(&mut matches);
        assert!(
            matches[0].entity.confidence.get() > 0.5,
            "global hint should trigger boost: got {}",
            matches[0].entity.confidence.get(),
        );
    }

    #[test]
    fn kind_specific_hint_takes_priority_over_global() {
        let r = rule(&[], 50, 0.1, 0.0);
        let text = "social something else medical 123 yes";
        let mut matches = vec![ssn_match(Some(r), 30, 33, 0.5)];
        let hints = vec![
            ContextHint {
                kind: Some(EntityKind::GovernmentId),
                keywords: vec!["social".into()],
                boost: Some(0.5),
                ..ContextHint::default()
            },
            ContextHint {
                kind: None,
                keywords: vec!["medical".into()],
                boost: Some(0.05),
                ..ContextHint::default()
            },
        ];
        ContextEnhancer::new(text, &hints).enhance(&mut matches);
        // Kind-specific bucket (boost=0.5) should win over global
        // (boost=0.05); both "social" and "medical" are in window
        // but only the kind-specific bucket is considered.
        assert!(
            (matches[0].entity.confidence.get() - 1.0).abs() < 1e-9,
            "kind-specific boost (0.5) should apply, clamped to 1.0: got {}",
            matches[0].entity.confidence.get(),
        );
    }

    #[test]
    fn hint_window_override_extends_search() {
        let mut text = "social ".to_string();
        text.push_str(&"x".repeat(200));
        text.push_str("MATCH");
        let match_start = text.len() - 5;
        let match_end = text.len();
        let r = rule(&[], 50, 0.1, 0.0);

        // Without override (window narrow) -> no boost.
        let mut matches = vec![ssn_match(Some(r.clone()), match_start, match_end, 0.5)];
        let narrow = vec![ContextHint {
            kind: Some(EntityKind::GovernmentId),
            keywords: vec!["social".into()],
            ..ContextHint::default()
        }];
        ContextEnhancer::new(&text, &narrow).enhance(&mut matches);
        assert_eq!(
            matches[0].entity.confidence.get(),
            0.5,
            "narrow window finds nothing"
        );

        // With window override -> boost applies.
        let mut matches = vec![ssn_match(Some(r), match_start, match_end, 0.5)];
        let wide = vec![ContextHint {
            kind: Some(EntityKind::GovernmentId),
            keywords: vec!["social".into()],
            window: Some(500),
            ..ContextHint::default()
        }];
        ContextEnhancer::new(&text, &wide).enhance(&mut matches);
        assert!(
            matches[0].entity.confidence.get() > 0.5,
            "window override should find keyword: got {}",
            matches[0].entity.confidence.get(),
        );
    }

    #[test]
    fn hint_boost_override_changes_magnitude() {
        let r = rule(&["ssn"], 30, 0.1, 0.0);
        let mut matches = vec![ssn_match(Some(r), 4, 7, 0.5)];
        let text = "ssn here at start";
        let hints = vec![ContextHint {
            kind: Some(EntityKind::GovernmentId),
            keywords: vec!["ssn".into()],
            boost: Some(0.4),
            ..ContextHint::default()
        }];
        ContextEnhancer::new(text, &hints).enhance(&mut matches);
        assert!(
            (matches[0].entity.confidence.get() - 0.9).abs() < 1e-9,
            "boost override should be 0.4: got {}",
            matches[0].entity.confidence.get(),
        );
    }

    #[test]
    fn empty_hint_keywords_disable_overrides() {
        let r = rule(&["ssn"], 30, 0.1, 0.0);
        let mut matches = vec![ssn_match(Some(r), 4, 7, 0.5)];
        let text = "ssn here at start";
        let hints = vec![ContextHint {
            kind: Some(EntityKind::GovernmentId),
            keywords: vec![],
            window: Some(500),
            boost: Some(0.9),
        }];
        ContextEnhancer::new(text, &hints).enhance(&mut matches);
        assert!(
            (matches[0].entity.confidence.get() - 0.6).abs() < 1e-9,
            "should use rule.boost=0.1, not hint.boost=0.9: got {}",
            matches[0].entity.confidence.get(),
        );
    }

    #[test]
    fn penalty_applies_when_no_keyword_anywhere() {
        let r = rule(&["ssn"], 30, 0.1, 0.2);
        let mut matches = vec![ssn_match(Some(r), 0, 5, 0.5)];
        let text = "nothing relevant here at all yes";
        ContextEnhancer::new(text, &[]).enhance(&mut matches);
        assert!(
            (matches[0].entity.confidence.get() - 0.3).abs() < 1e-9,
            "penalty should reduce confidence: got {}",
            matches[0].entity.confidence.get(),
        );
    }

    #[test]
    fn window_in_chars_safe_for_multibyte_text() {
        // Keyword "ssn" sits 4 chars left of the match, separated by
        // 3 emojis (each 4 bytes in UTF-8). A byte-counting window
        // of 4 would land mid-emoji and panic; the char-counting
        // window walks scalar boundaries and finds the keyword.
        let text = "ssn 🔥🔥🔥 MATCH";
        let match_start = text.rfind("MATCH").unwrap();
        let match_end = match_start + "MATCH".len();
        let r = rule(&["ssn"], 8, 0.1, 0.0); // window = 8 chars
        let mut matches = vec![ssn_match(Some(r), match_start, match_end, 0.5)];
        ContextEnhancer::new(text, &[]).enhance(&mut matches);
        assert!(
            (matches[0].entity.confidence.get() - 0.6).abs() < 1e-9,
            "char-window should include 'ssn' across multi-byte chars: got {}",
            matches[0].entity.confidence.get(),
        );
    }

    #[test]
    fn penalty_skipped_when_hint_keyword_matches() {
        let r = rule(&["ssn"], 30, 0.1, 0.2);
        let mut matches = vec![ssn_match(Some(r), 10, 15, 0.5)];
        let text = "social security is the relevant thing";
        let hints = vec![ContextHint {
            kind: Some(EntityKind::GovernmentId),
            keywords: vec!["social".into()],
            ..ContextHint::default()
        }];
        ContextEnhancer::new(text, &hints).enhance(&mut matches);
        assert!(
            matches[0].entity.confidence.get() > 0.5,
            "hint match should boost, not penalize: got {}",
            matches[0].entity.confidence.get(),
        );
    }
}
