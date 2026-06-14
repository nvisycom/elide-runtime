//! [`BoostRule`]: per-label keyword-boost rule.
//!
//! One rule per [`EntityLabelRef`] declares the keyword set that
//! lifts confidence when one of those keywords appears within
//! `prefix_words` words before or `suffix_words` words after an
//! entity carrying that label. The window radii and the additive
//! `boost` are resolved at rule construction time — there are no
//! per-source overrides at apply time.
//!
//! Producers (the pattern crate today, future NER/LLM/custom
//! recognizer authors) hand the engine a `Vec<BoostRule>` keyed by
//! label. When several rules contribute to the same label (e.g.
//! two different SSN detectors both contributing to
//! `GOVERNMENT_ID`), the engine merges them by union of keywords —
//! see [`BoostRule::merge`].
//!
//! [`EntityLabelRef`]: nvisy_core::entity::EntityLabelRef

use std::collections::HashSet;

use hipstr::HipStr;
use nvisy_core::entity::EntityLabelRef;
use nvisy_core::primitive::Confidence;

/// Default window radius in words *before* an entity match.
/// Mirrors Presidio's `context_prefix_count = 5`.
pub const DEFAULT_PREFIX_WORDS: usize = 5;

/// Default window radius in words *after* an entity match. Set
/// equal to [`DEFAULT_PREFIX_WORDS`] so trailing context like
/// "123-45-6789 (social security)" boosts the same as leading
/// context. Presidio defaults `context_suffix_count` to `0`; we
/// pick symmetric defaults because operators rarely realize the
/// asymmetry exists, and one-sided windows surprise people.
pub const DEFAULT_SUFFIX_WORDS: usize = 5;

/// Default additive boost applied when a keyword fires. Matches
/// Presidio's `context_similarity_factor = 0.35`.
pub const DEFAULT_BOOST: f64 = 0.35;

/// Per-label boost rule the [`Enhancer`] applies at runtime.
///
/// [`Enhancer`]: super::Enhancer
#[derive(Debug, Clone, PartialEq)]
pub struct BoostRule {
    /// Entity label this rule applies to. Each emitted
    /// `Entity<Text>` whose [`label`] matches is checked against
    /// this rule's keywords.
    ///
    /// [`label`]: nvisy_core::entity::Entity::label
    pub label: EntityLabelRef,
    /// Keywords whose presence near a match lifts the entity's
    /// confidence. Stored as [`HipStr`] for cheap clones across
    /// per-pass rule sets.
    pub keywords: Vec<HipStr<'static>>,
    /// Window radius in words *before* the entity's match.
    /// Counted against the token artifact on
    /// `RecognizerInput.artifacts` when present, or via Unicode
    /// word segmentation of the source text otherwise.
    pub prefix_words: usize,
    /// Window radius in words *after* the entity's match. Same
    /// source as [`prefix_words`].
    ///
    /// [`prefix_words`]: Self::prefix_words
    pub suffix_words: usize,
    /// Additive boost applied to the entity's confidence when a
    /// keyword fires. Clamped at the [`Confidence`] ceiling on
    /// apply.
    pub boost: Confidence,
}

impl BoostRule {
    /// Construct a rule for `label` with explicit window radii
    /// and `boost`. Most callers want [`BoostRule::for_label`]
    /// instead — it bakes in the default window / boost values.
    #[must_use]
    pub fn new(
        label: EntityLabelRef,
        keywords: impl IntoIterator<Item = impl Into<HipStr<'static>>>,
        prefix_words: usize,
        suffix_words: usize,
        boost: Confidence,
    ) -> Self {
        Self {
            label,
            keywords: keywords.into_iter().map(Into::into).collect(),
            prefix_words,
            suffix_words,
            boost,
        }
    }

    /// Construct a rule for `label` using the crate's default
    /// [`prefix_words`], [`suffix_words`], and [`boost`]
    /// constants. The common case — recognizers building their
    /// own boost rules from declared keywords don't need to
    /// think about tuning knobs.
    ///
    /// [`prefix_words`]: DEFAULT_PREFIX_WORDS
    /// [`suffix_words`]: DEFAULT_SUFFIX_WORDS
    /// [`boost`]: DEFAULT_BOOST
    #[must_use]
    pub fn for_label(
        label: EntityLabelRef,
        keywords: impl IntoIterator<Item = impl Into<HipStr<'static>>>,
    ) -> Self {
        Self::new(
            label,
            keywords,
            DEFAULT_PREFIX_WORDS,
            DEFAULT_SUFFIX_WORDS,
            Confidence::clamped(DEFAULT_BOOST),
        )
    }

    /// Merge `other` into this rule by extending the keyword set
    /// with any keywords not already present. Window radii and
    /// `boost` are kept from `self` — callers that need different
    /// values per source should construct independent rules and
    /// keep them separate.
    ///
    /// # Panics
    ///
    /// Debug-asserts when the labels differ. Merging across labels
    /// is a caller bug — rules are keyed by label and the engine
    /// looks them up by label.
    pub fn merge(&mut self, other: BoostRule) {
        debug_assert_eq!(
            self.label, other.label,
            "BoostRule::merge requires matching labels",
        );
        let existing: HashSet<&str> = self.keywords.iter().map(HipStr::as_str).collect();
        let additions: Vec<HipStr<'static>> = other
            .keywords
            .into_iter()
            .filter(|kw| !existing.contains(kw.as_str()))
            .collect();
        self.keywords.extend(additions);
    }
}
