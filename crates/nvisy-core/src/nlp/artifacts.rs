//! [`NlpArtifacts`]: the shared NLP bag every text recognizer reads
//! from when the orchestrator opted into a shared-pass pipeline.
//!
//! See the [module docs] for the producer/consumer split and
//! the conceptual model. This file just defines the data shape.
//!
//! [module docs]: super

use super::{RawNerSpan, StopwordSet, Tokens};
use crate::primitive::LanguageDetection;

/// One scan's worth of shared NLP output.
///
/// Bundles everything an `NlpEngine` produced for one text scan.
/// All fields are populated according to the engine's advertised
/// [`NlpCapabilities`]; fields the engine
/// doesn't produce hold their type's empty value
/// (`Tokens::empty()`, `Vec::new()`, `StopwordSet::empty()`).
///
/// Coordinate space for [`tokens`],
/// [`ner`], and any byte-offset queries is the source
/// text the engine was called with — the same coordinate space as
/// [`Entity::location`]
/// for text entities.
///
/// `NlpArtifacts` is constructed by an `NlpEngine` (declared in
/// `nvisy-ner`) and typically wrapped in an `Arc` by the
/// orchestrator so it can be fanned out to every recognizer
/// without cloning the underlying token / NER vectors.
///
/// [`NlpCapabilities`]: super::NlpCapabilities
/// [`tokens`]: Self::tokens
/// [`ner`]: Self::ner
/// [`Entity::location`]: crate::entity::Entity::location
#[derive(Debug, Clone, Default)]
pub struct NlpArtifacts {
    /// Languages the engine resolved for the input. Empty when the
    /// engine couldn't decide *and* no caller language was
    /// asserted.
    pub languages: Vec<LanguageDetection>,
    /// Tokenized text. Empty when the engine has no tokenizer.
    pub tokens: Tokens,
    /// NER spans in raw, pre-normalization form. Empty when the
    /// engine has no NER model.
    pub ner: Vec<RawNerSpan>,
    /// Stopword set resolved for the dominant language. Empty when
    /// the engine has no stopword list.
    pub stopwords: StopwordSet,
}

impl NlpArtifacts {
    /// Construct an artifact with only the language field
    /// populated. Used by language-only engines.
    pub fn language_only(languages: Vec<LanguageDetection>) -> Self {
        Self {
            languages,
            tokens: Tokens::empty(),
            ner: Vec::new(),
            stopwords: StopwordSet::empty(),
        }
    }

    /// The language covering the most bytes of the source text,
    /// breaking ties on detector confidence.
    ///
    /// Mirrors the logic that used to live on `nvisy-ner`'s
    /// `Artifacts::dominant_language`: monolingual docs return the
    /// single detection; mixed-language docs return the
    /// largest-coverage span; caller-asserted languages (no
    /// `span`) are treated as covering the whole document and
    /// therefore win against any one region.
    ///
    /// Returns `None` iff [`languages`] is empty.
    ///
    /// [`languages`]: Self::languages
    pub fn dominant_language(&self) -> Option<&LanguageDetection> {
        self.languages.iter().max_by(|a, b| {
            span_bytes(a)
                .cmp(&span_bytes(b))
                .then_with(|| confidence_key(a).total_cmp(&confidence_key(b)))
        })
    }
}

fn span_bytes(d: &LanguageDetection) -> usize {
    match d.span {
        Some(s) => s.end.saturating_sub(s.start),
        None => usize::MAX,
    }
}

fn confidence_key(d: &LanguageDetection) -> f64 {
    d.confidence.map(|c| c.get()).unwrap_or(f64::NEG_INFINITY)
}
