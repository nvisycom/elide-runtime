//! [`Enhancer`]: post-recognition keyword-boost pass for any
//! [`Entity<Text>`] regardless of which recognizer produced it.

use std::collections::HashMap;

use nvisy_core::entity::{Entity, EntityLabelRef, TrailStep};
use nvisy_core::modality::Text;
use unicode_segmentation::UnicodeSegmentation;

use super::matcher::KeywordMatcher;
use super::rule::BoostRule;
use super::tokens::Token;

/// Source name stamped onto every refinement [`TrailStep`] the
/// enhancer appends.
const TRAIL_SOURCE: &str = "context";

/// Post-recognition enhancer. Holds a label-keyed [`BoostRule`]
/// map plus the keyword-matching strategy, and lifts the
/// confidence of each text entity whose label has a rule and
/// whose surrounding word window contains one of the rule's
/// keywords.
///
/// Construct via [`Enhancer::new`]. Rules are passed in by value;
/// duplicates for the same label are merged via
/// [`BoostRule::merge`] (union of keywords; window radii / `boost`
/// kept from the first-seen rule).
///
/// The matcher defaults are picked by the engine that constructs
/// the enhancer: [`SubstringMatcher`] when no upstream NLP engine
/// produces tokens, [`LemmaMatcher`] when one does.
///
/// [`SubstringMatcher`]: super::SubstringMatcher
/// [`LemmaMatcher`]: super::LemmaMatcher
pub struct Enhancer {
    rules: HashMap<EntityLabelRef, BoostRule>,
    matcher: Box<dyn KeywordMatcher>,
}

impl Enhancer {
    /// Construct from a rule iterator and matcher. Rules sharing
    /// the same label are merged via [`BoostRule::merge`].
    pub fn new(
        rules: impl IntoIterator<Item = BoostRule>,
        matcher: Box<dyn KeywordMatcher>,
    ) -> Self {
        let mut map: HashMap<EntityLabelRef, BoostRule> = HashMap::new();
        for rule in rules {
            match map.get_mut(&rule.label) {
                Some(existing) => existing.merge(rule),
                None => {
                    map.insert(rule.label.clone(), rule);
                }
            }
        }
        Self {
            rules: map,
            matcher,
        }
    }

    /// `true` when no rules are registered. Engine code uses this
    /// to short-circuit calls to [`enhance`] entirely.
    ///
    /// [`enhance`]: Self::enhance
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Number of distinct labels with rules.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Apply boost rules to `entities` in place. For each entity:
    /// look up the rule for its label, walk a window of
    /// `prefix_words` words before and `suffix_words` words after
    /// the entity's location, ask the matcher whether any keyword
    /// fires, and on a hit lift confidence by the rule's `boost`
    /// (saturating at the [`Confidence`] ceiling) plus append a
    /// [`Refinement`] trail step.
    ///
    /// `tokens` is the optional token artifact produced by an
    /// upstream NLP engine. When present, words are counted
    /// against the token stream; when absent, words are derived
    /// from the source text via Unicode word segmentation.
    ///
    /// [`Confidence`]: nvisy_core::primitive::Confidence
    /// [`Refinement`]: nvisy_core::entity::TrailStepKind::Refinement
    pub fn enhance(&self, entities: &mut [Entity<Text>], text: &str, tokens: Option<&[Token]>) {
        if self.rules.is_empty() {
            return;
        }
        for entity in entities {
            self.enhance_one(entity, text, tokens);
        }
    }

    fn enhance_one(&self, entity: &mut Entity<Text>, text: &str, tokens: Option<&[Token]>) {
        let Some(rule) = self.rules.get(&entity.label) else {
            return;
        };
        if rule.keywords.is_empty() {
            return;
        }

        let start = entity.location.start;
        let end = entity.location.end;

        // Prefer the token stream when the producer reached this
        // entity. Fall back to the word-segmented substring window
        // whenever the token slice would be empty — that covers
        // `tokens: None`, `tokens: Some(&[])`, and the "tokens
        // present but none overlap the entity" case (e.g. NLP
        // engine only tokenized part of the document).
        let token_slice = tokens
            .map(|toks| slice_tokens_around(toks, start, end, rule.prefix_words, rule.suffix_words))
            .unwrap_or(&[]);
        let (snippet, tokens_in_window): (&str, &[Token]) = if token_slice.is_empty() {
            let snippet = word_window(text, start, end, rule.prefix_words, rule.suffix_words);
            (snippet, &[])
        } else {
            let snippet = token_span(text, token_slice, start, end);
            (snippet, token_slice)
        };

        if !self
            .matcher
            .any_match(snippet, tokens_in_window, &rule.keywords)
        {
            return;
        }

        let original = entity.confidence;
        let adjusted = original.saturating_add(rule.boost.get());
        if adjusted == original {
            return;
        }
        entity.confidence = adjusted;

        entity.trail.push(TrailStep::refinement(
            TRAIL_SOURCE,
            original,
            adjusted,
            format!(
                "context keyword near `{}` (+{:.3})",
                entity.label.as_str(),
                rule.boost.get(),
            ),
        ));
    }
}

/// Walk `prefix` words before `[start, end)` and `suffix` words
/// after, via Unicode word segmentation, and return the spanning
/// substring (including any non-word whitespace and punctuation
/// between words). The returned slice covers `[start, end)` itself
/// plus the prefix / suffix words; the entity's own bytes are
/// always inside.
fn word_window(text: &str, start: usize, end: usize, prefix: usize, suffix: usize) -> &str {
    let prefix_text = &text[..start.min(text.len())];
    let suffix_text = &text[end.min(text.len())..];

    // `unicode_word_indices` yields `(byte_offset, word_str)` for
    // every "word" (alphanumeric run) in source order. Take the
    // last `prefix` on the prefix side, the first `suffix` on the
    // suffix side, and compute the spanning byte range.
    let prefix_words: Vec<(usize, &str)> = prefix_text.unicode_word_indices().collect();
    let prefix_take = prefix_words.len().saturating_sub(prefix);
    let prefix_byte = prefix_words
        .get(prefix_take)
        .map(|(idx, _)| *idx)
        .unwrap_or(start.min(text.len()));

    let suffix_byte = if suffix == 0 {
        end.min(text.len())
    } else {
        suffix_text
            .unicode_word_indices()
            .nth(suffix - 1)
            .map(|(idx, word)| end + idx + word.len())
            .unwrap_or(text.len())
    };

    let lo = floor_char_boundary(text, prefix_byte);
    let hi = ceil_char_boundary(text, suffix_byte.min(text.len()));
    &text[lo..hi]
}

fn floor_char_boundary(s: &str, mut pos: usize) -> usize {
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

fn ceil_char_boundary(s: &str, mut pos: usize) -> usize {
    while pos < s.len() && !s.is_char_boundary(pos) {
        pos += 1;
    }
    pos
}

/// Slice tokens by *count*: take `prefix` tokens before the first
/// token overlapping `[start, end)` and `suffix` tokens after the
/// last. The returned slice is contiguous.
fn slice_tokens_around(
    tokens: &[Token],
    start: usize,
    end: usize,
    prefix: usize,
    suffix: usize,
) -> &[Token] {
    if tokens.is_empty() {
        return &[];
    }
    // First token whose `offset.end > start` overlaps or follows the entity.
    let first_overlap = tokens.partition_point(|t| t.offset.end <= start);
    // One past the last token whose `offset.start < end` overlaps the entity.
    let last_overlap = tokens.partition_point(|t| t.offset.start < end);
    let lo = first_overlap.saturating_sub(prefix);
    let hi = (last_overlap + suffix).min(tokens.len());
    if lo >= hi {
        return &[];
    }
    &tokens[lo..hi]
}

/// Spanning substring covering `tokens` plus the entity itself.
/// Used to give the matcher a contiguous text window when slicing
/// against the token stream.
///
/// Precondition: `tokens` is non-empty. Callers must take the
/// `word_window` fallback path when their token slice is empty —
/// see `Enhancer::enhance_one`.
fn token_span<'a>(text: &'a str, tokens: &[Token], start: usize, end: usize) -> &'a str {
    debug_assert!(!tokens.is_empty(), "token_span requires non-empty slice");
    let lo = tokens[0].offset.start.min(start);
    let hi = tokens[tokens.len() - 1].offset.end.max(end);
    let lo = floor_char_boundary(text, lo.min(text.len()));
    let hi = ceil_char_boundary(text, hi.min(text.len()));
    &text[lo..hi]
}

#[cfg(test)]
mod tests {
    use nvisy_core::entity::{
        EntityLabelRef, PatternProvenance, TrailProvenance, TrailStepKind, builtins,
    };
    use nvisy_core::modality::{Text, TextLocation};
    use nvisy_core::primitive::Confidence;

    use super::*;
    use crate::SubstringMatcher;

    fn govid_label() -> EntityLabelRef {
        builtins::GOVERNMENT_ID.label_ref()
    }

    fn person_label() -> EntityLabelRef {
        builtins::PERSON_NAME.label_ref()
    }

    fn entity(label: EntityLabelRef, start: usize, end: usize, score: f64) -> Entity<Text> {
        let confidence = Confidence::new(score).unwrap();
        let step = TrailStep::recognition(
            "test",
            confidence,
            TrailProvenance::Pattern(PatternProvenance::DenyList),
            "test fixture",
        );
        Entity::builder()
            .with_label(label)
            .with_trail(vec![step])
            .with_confidence(confidence)
            .with_location(TextLocation::new(start, end))
            .build()
            .expect("entity builds")
    }

    fn enhancer(rules: Vec<BoostRule>) -> Enhancer {
        Enhancer::new(rules, Box::new(SubstringMatcher))
    }

    fn rule(
        label: EntityLabelRef,
        keywords: &[&'static str],
        prefix: usize,
        suffix: usize,
        boost: f64,
    ) -> BoostRule {
        BoostRule::new(
            label,
            keywords.iter().copied(),
            prefix,
            suffix,
            Confidence::clamped(boost),
        )
    }

    #[test]
    fn boosts_entity_when_keyword_in_word_window() {
        let enhancer = enhancer(vec![rule(
            govid_label(),
            &["ssn", "social security"],
            5,
            5,
            0.2,
        )]);
        let text = "Your SSN: 123-45-6789";
        let mut entities = vec![entity(govid_label(), 10, 21, 0.6)];
        enhancer.enhance(&mut entities, text, None);
        assert!(entities[0].confidence.get() > 0.6);
        assert!(
            entities[0]
                .trail
                .iter()
                .any(|s| matches!(s.kind, TrailStepKind::Refinement)),
        );
    }

    #[test]
    fn boosts_entity_when_keyword_in_suffix() {
        let enhancer = enhancer(vec![rule(govid_label(), &["social"], 0, 5, 0.2)]);
        let text = "123-45-6789 (social security number)";
        let mut entities = vec![entity(govid_label(), 0, 11, 0.6)];
        enhancer.enhance(&mut entities, text, None);
        assert!(
            entities[0].confidence.get() > 0.6,
            "trailing keyword within suffix window should boost",
        );
    }

    #[test]
    fn suffix_zero_ignores_trailing_keyword() {
        // Prefix-only: trailing keyword must not boost.
        let enhancer = enhancer(vec![rule(govid_label(), &["social"], 5, 0, 0.2)]);
        let text = "123-45-6789 (social security number)";
        let mut entities = vec![entity(govid_label(), 0, 11, 0.6)];
        let before = entities[0].confidence.get();
        enhancer.enhance(&mut entities, text, None);
        assert_eq!(entities[0].confidence.get(), before);
    }

    #[test]
    fn skips_entity_with_no_rule_for_label() {
        let enhancer = enhancer(vec![rule(govid_label(), &["ssn"], 5, 5, 0.2)]);
        let text = "Mr. Smith is named in the report.";
        let mut entities = vec![entity(person_label(), 4, 9, 0.5)];
        let before = entities[0].confidence.get();
        enhancer.enhance(&mut entities, text, None);
        assert_eq!(entities[0].confidence.get(), before);
    }

    #[test]
    fn window_bounds_the_search() {
        // 2-word prefix / 2-word suffix: "far_keyword" is at the
        // start; the entity is after many filler words.
        let enhancer = enhancer(vec![rule(govid_label(), &["far_keyword"], 2, 2, 0.2)]);
        let text = "far_keyword here is some filler between the keyword and XYZ here";
        let xyz_start = text.find("XYZ").unwrap();
        let xyz_end = xyz_start + "XYZ".len();
        let mut entities = vec![entity(govid_label(), xyz_start, xyz_end, 0.6)];
        let before = entities[0].confidence.get();
        enhancer.enhance(&mut entities, text, None);
        assert_eq!(entities[0].confidence.get(), before);
    }

    #[test]
    fn boost_saturates_at_one() {
        let enhancer = enhancer(vec![rule(govid_label(), &["here"], 5, 5, 0.9)]);
        let text = "the value is right here in plain sight";
        let mut entities = vec![entity(govid_label(), 16, 21, 0.95)];
        enhancer.enhance(&mut entities, text, None);
        assert!((entities[0].confidence.get() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn duplicate_label_rules_merge_keywords() {
        // Two rules for the same label, each contributing a
        // distinct keyword. The merged rule must trigger boosts
        // for matches near keywords from *either* original source,
        // proving the keyword union survived the merge (not just
        // last-write-wins).
        let make_enhancer = || {
            enhancer(vec![
                rule(govid_label(), &["ssn"], 5, 5, 0.2),
                rule(govid_label(), &["tax id"], 5, 5, 0.2),
            ])
        };
        assert_eq!(make_enhancer().len(), 1);

        // Keyword only from the first rule.
        let ssn_only = "ssn: 123-45-6789";
        let ssn_entity_start = ssn_only.find("123").unwrap();
        let ssn_entity_end = ssn_entity_start + "123-45-6789".len();
        let mut from_first = vec![entity(govid_label(), ssn_entity_start, ssn_entity_end, 0.6)];
        make_enhancer().enhance(&mut from_first, ssn_only, None);
        assert!(
            from_first[0].confidence.get() > 0.6,
            "keyword `ssn` from the first rule must still boost after merge",
        );

        // Keyword only from the second rule.
        let taxid_only = "tax id: 987-65-4329";
        let tax_entity_start = taxid_only.find("987").unwrap();
        let tax_entity_end = tax_entity_start + "987-65-4329".len();
        let mut from_second = vec![entity(govid_label(), tax_entity_start, tax_entity_end, 0.6)];
        make_enhancer().enhance(&mut from_second, taxid_only, None);
        assert!(
            from_second[0].confidence.get() > 0.6,
            "keyword `tax id` from the second rule must still boost after merge",
        );
    }

    #[test]
    fn word_window_handles_unicode() {
        // 3-word prefix reaches "café" past "naïve" and "resume".
        let enhancer = enhancer(vec![rule(govid_label(), &["café"], 3, 0, 0.2)]);
        let text = "café naïve resume — 123-45-6789";
        let entity_start = text.find("123").unwrap();
        let entity_end = entity_start + "123-45-6789".len();
        let mut entities = vec![entity(govid_label(), entity_start, entity_end, 0.6)];
        enhancer.enhance(&mut entities, text, None);
        assert!(
            entities[0].confidence.get() > 0.6,
            "unicode word should be reachable within 3-word prefix",
        );
    }

    #[test]
    fn word_window_excludes_too_distant_unicode() {
        // 2-word prefix: "café" is the 3rd word before the entity.
        let enhancer = enhancer(vec![rule(govid_label(), &["café"], 2, 0, 0.2)]);
        let text = "café naïve resume — 123-45-6789";
        let entity_start = text.find("123").unwrap();
        let entity_end = entity_start + "123-45-6789".len();
        let mut entities = vec![entity(govid_label(), entity_start, entity_end, 0.6)];
        let before = entities[0].confidence.get();
        enhancer.enhance(&mut entities, text, None);
        assert_eq!(entities[0].confidence.get(), before);
    }

    #[test]
    fn empty_tokens_slice_matches_none_behaviour() {
        // Keyword sits in the prefix word-window but outside the
        // entity bytes. With the empty-slice fix, `Some(&[])` must
        // not collapse the snippet to the entity bytes — it should
        // fall back to the word-window path just like `None`.
        let enhancer = enhancer(vec![rule(govid_label(), &["ssn"], 5, 5, 0.2)]);
        let text = "Your SSN: 123-45-6789";
        let mut from_none = vec![entity(govid_label(), 10, 21, 0.6)];
        let mut from_empty = vec![entity(govid_label(), 10, 21, 0.6)];
        enhancer.enhance(&mut from_none, text, None);
        enhancer.enhance(&mut from_empty, text, Some(&[]));
        assert_eq!(
            from_none[0].confidence.get(),
            from_empty[0].confidence.get(),
            "Some(&[]) must behave identically to None",
        );
        assert!(
            from_empty[0].confidence.get() > 0.6,
            "empty tokens slice must still allow the word-window fallback to boost",
        );
    }

    #[test]
    fn token_path_counts_words_against_token_stream() {
        // 1-word prefix, 0-word suffix: the only word the
        // prefix reaches is the immediate predecessor token
        // "Your". The tokenizer here treats "social security"
        // as a single compound token outside the window, so the
        // keyword "social security" must NOT fire — unlike a
        // hypothetical caller that gave it the word-window path,
        // which would split on whitespace.
        let enhancer = enhancer(vec![rule(govid_label(), &["social security"], 1, 0, 0.2)]);
        let text = "social security: Your 123-45-6789";
        let entity_start = text.find("123").unwrap();
        let entity_end = entity_start + "123-45-6789".len();
        let tokens: Vec<Token> = vec![
            Token::from_text("social security", 0..15),
            Token::from_text("Your", 17..21),
            Token::from_text("123-45-6789", 22..33),
        ];
        let mut entities = vec![entity(govid_label(), entity_start, entity_end, 0.6)];
        let before = entities[0].confidence.get();
        enhancer.enhance(&mut entities, text, Some(&tokens));
        assert_eq!(
            entities[0].confidence.get(),
            before,
            "1-word prefix should not reach the `social security` token two positions back",
        );
    }

    #[test]
    fn token_path_boosts_when_keyword_within_token_window() {
        // Same tokens, 2-word prefix: now the `social security`
        // token is reachable and the boost fires.
        let enhancer = enhancer(vec![rule(govid_label(), &["social security"], 2, 0, 0.2)]);
        let text = "social security: Your 123-45-6789";
        let entity_start = text.find("123").unwrap();
        let entity_end = entity_start + "123-45-6789".len();
        let tokens: Vec<Token> = vec![
            Token::from_text("social security", 0..15),
            Token::from_text("Your", 17..21),
            Token::from_text("123-45-6789", 22..33),
        ];
        let mut entities = vec![entity(govid_label(), entity_start, entity_end, 0.6)];
        enhancer.enhance(&mut entities, text, Some(&tokens));
        assert!(
            entities[0].confidence.get() > 0.6,
            "2-word prefix should reach the `social security` token",
        );
    }

    #[test]
    fn lemma_matcher_boosts_on_morphological_variant() {
        // Substring matcher would miss `running` for keyword
        // `run`. Lemma matcher reads the lemma directly off the
        // token and boosts.
        let enhancer = Enhancer::new(
            vec![rule(govid_label(), &["run"], 5, 5, 0.2)],
            Box::new(crate::LemmaMatcher),
        );
        let text = "They were running 123-45-6789 across the system";
        let entity_start = text.find("123").unwrap();
        let entity_end = entity_start + "123-45-6789".len();
        let tokens: Vec<Token> = vec![
            Token::from_text("They", 0..4),
            Token::from_text("were", 5..9),
            Token::from_text("running", 10..17).with_lemma("run"),
            Token::from_text("123-45-6789", 18..29),
            Token::from_text("across", 30..36),
            Token::from_text("the", 37..40),
            Token::from_text("system", 41..47),
        ];
        let mut entities = vec![entity(govid_label(), entity_start, entity_end, 0.6)];
        enhancer.enhance(&mut entities, text, Some(&tokens));
        assert!(
            entities[0].confidence.get() > 0.6,
            "lemma matcher should match `run` against the `running` token's lemma",
        );
        assert!(
            entities[0]
                .trail
                .iter()
                .any(|s| matches!(s.kind, TrailStepKind::Refinement)),
        );
    }

    #[test]
    fn tokens_with_no_overlap_fall_back_to_word_window() {
        // Tokens cover the first half of the document; the entity
        // is in the second half, outside any token's range.
        // Without the fallback the token slice would be empty and
        // the snippet would collapse to entity bytes. With the
        // fallback, the word-window path reaches the keyword.
        let enhancer = enhancer(vec![rule(govid_label(), &["ssn"], 5, 5, 0.2)]);
        let text = "First half of the document. Your SSN: 123-45-6789";
        let entity_start = text.find("123").unwrap();
        let entity_end = entity_start + "123-45-6789".len();
        // Tokens that cover only the first sentence.
        let tokens: Vec<Token> = vec![
            Token::from_text("First", 0..5),
            Token::from_text("half", 6..10),
            Token::from_text("of", 11..13),
            Token::from_text("the", 14..17),
            Token::from_text("document", 18..26),
        ];
        let mut entities = vec![entity(govid_label(), entity_start, entity_end, 0.6)];
        enhancer.enhance(&mut entities, text, Some(&tokens));
        assert!(
            entities[0].confidence.get() > 0.6,
            "tokens that don't overlap the entity must fall back to the word window",
        );
    }
}
