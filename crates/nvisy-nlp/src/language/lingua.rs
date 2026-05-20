//! [`LinguaLanguageDetector`] — wraps the `lingua` crate's
//! [`lingua::LanguageDetector`].
//!
//! Constructors mirror [`lingua::LanguageDetectorBuilder`]:
//!
//! - [`for_languages`] — restrict to a known set. **Preferred when
//!   the corpus language is known**; lingua is more accurate when
//!   the candidate set is narrowed.
//! - [`for_all_languages`] — consider every language compiled into
//!   the `lingua` crate's feature set. Use only when the language is
//!   genuinely unknown.
//!
//! Neither constructor enables `low_accuracy_mode`; lingua's default
//! (highest accuracy) is in effect.
//!
//! [`for_languages`]: LinguaLanguageDetector::for_languages
//! [`for_all_languages`]: LinguaLanguageDetector::for_all_languages

use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::{fmt, str::FromStr};

use lingua::{IsoCode639_1, Language, LanguageDetector as LinguaDetector, LanguageDetectorBuilder};
use nvisy_ontology::primitive::{Confidence, LanguageTag};

use super::{LanguageDetection, LanguageDetector, LanguageProvenance, LanguageSpan};
use crate::error::Result;

/// A [`LanguageDetector`] backed by [`lingua`].
///
/// [`lingua`]: https://crates.io/crates/lingua
pub struct LinguaLanguageDetector {
    inner: LinguaDetector,
    /// Languages the detector was constructed with. Retained so that
    /// per-call candidate-set restriction
    /// ([`LanguageDetector::detect_in`]) can intersect with the
    /// configured set instead of widening it.
    configured: Vec<Language>,
}

impl LinguaLanguageDetector {
    /// Construct a detector restricted to the given languages.
    ///
    /// This is the **recommended constructor** when the deployment
    /// knows which languages it processes — lingua is more accurate
    /// and uses less memory with a narrower candidate set.
    ///
    /// Unrecognised tags (no matching ISO 639-1 primary subtag among
    /// lingua's supported languages) are silently skipped. Returns
    /// `None` when no input tag is recognised — a detector with zero
    /// candidates is useless.
    ///
    /// Whichever languages are enabled via cargo features on the
    /// `lingua` dependency must also be requested here for the
    /// builder to accept them.
    pub fn for_languages(tags: &[LanguageTag]) -> Option<Self> {
        let langs = tags_to_languages(tags);
        if langs.is_empty() {
            return None;
        }
        let inner = LanguageDetectorBuilder::from_languages(&langs).build();
        Some(Self {
            inner,
            configured: langs,
        })
    }

    /// Construct a detector considering every language compiled into
    /// the `lingua` crate's feature set.
    ///
    /// **Use only when the corpus language is genuinely unknown.** A
    /// restricted set via [`for_languages`] gives better accuracy
    /// and lower memory use when you do know the candidate languages.
    ///
    /// The actual languages considered depend on which `lingua`
    /// feature flags are enabled — e.g. only `english` by default in
    /// the nvisy workspace.
    ///
    /// [`for_languages`]: Self::for_languages
    pub fn for_all_languages() -> Self {
        let inner = LanguageDetectorBuilder::from_all_languages().build();
        Self {
            inner,
            configured: Language::all().into_iter().collect(),
        }
    }

    fn lingua_to_tag(lang: Language) -> Option<LanguageTag> {
        let iso = lang.iso_code_639_1().to_string();
        match iso.parse() {
            Ok(tag) => Some(tag),
            Err(e) => {
                warn_once_unmappable(&iso, &e.to_string());
                None
            }
        }
    }

    /// Run lingua's multi-language detector and map every returned
    /// region to a [`LanguageDetection`] with [`LanguageSpan`] set.
    /// Used by both [`detect`] (configured detector) and
    /// [`detect_in`] (per-call restricted detector).
    ///
    /// [`detect`]: <Self as LanguageDetector>::detect
    /// [`detect_in`]: <Self as LanguageDetector>::detect_in
    fn detect_with(detector: &LinguaDetector, text: &str) -> Result<Vec<LanguageDetection>> {
        let detections = detector
            .detect_multiple_languages_of(text)
            .into_iter()
            .filter_map(|result| {
                let language = Self::lingua_to_tag(result.language())?;
                let raw_confidence = detector.compute_language_confidence(text, result.language());
                let confidence = Confidence::new(raw_confidence.clamp(0.0, 1.0));
                Some(LanguageDetection {
                    language,
                    confidence,
                    provenance: LanguageProvenance::Detected,
                    span: Some(LanguageSpan {
                        start: result.start_index(),
                        end: result.end_index(),
                    }),
                })
            })
            .collect();
        Ok(detections)
    }
}

/// Cache of ISO codes we've already logged an "unmappable" warning
/// for, so a hot detection loop doesn't spam the log with the same
/// failure once per call. Lingua's code set is finite and fixed —
/// real failures here are deterministic.
fn warn_once_unmappable(iso: &str, error: &str) {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    let mut guard = match seen.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.insert(iso.to_owned()) {
        tracing::warn!(
            target: "nvisy_nlp::language::lingua",
            iso_code = %iso,
            error = %error,
            "lingua ISO 639-1 code did not parse as a BCP-47 LanguageTag (logged once per process)",
        );
    }
}

fn tags_to_languages(tags: &[LanguageTag]) -> Vec<Language> {
    tags.iter()
        .filter_map(|t| IsoCode639_1::from_str(t.primary_language()).ok())
        .map(|iso| Language::from_iso_code_639_1(&iso))
        .collect()
}

impl LanguageDetector for LinguaLanguageDetector {
    fn detect(&self, text: &str) -> Result<Vec<LanguageDetection>> {
        Self::detect_with(&self.inner, text)
    }

    fn detect_in(&self, text: &str, candidates: &[LanguageTag]) -> Result<Vec<LanguageDetection>> {
        if candidates.is_empty() {
            return self.detect(text);
        }
        let requested = tags_to_languages(candidates);
        let intersection: Vec<Language> = self
            .configured
            .iter()
            .copied()
            .filter(|l| requested.contains(l))
            .collect();
        match intersection.len() {
            0 => Ok(Vec::new()),
            n if n == self.configured.len() => self.detect(text),
            _ => {
                let restricted = LanguageDetectorBuilder::from_languages(&intersection).build();
                Self::detect_with(&restricted, text)
            }
        }
    }
}

impl fmt::Debug for LinguaLanguageDetector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinguaLanguageDetector").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn english_only() -> LinguaLanguageDetector {
        let tags = ["en".parse().unwrap()];
        LinguaLanguageDetector::for_languages(&tags).expect("english is enabled")
    }

    #[test]
    fn detects_english_sentence() {
        let det = english_only();
        let detections = det
            .detect("The quick brown fox jumps over the lazy dog.")
            .unwrap();
        assert!(!detections.is_empty(), "expected at least one detection");
        let first = &detections[0];
        assert_eq!(first.language.primary_language(), "en");
        let conf = first.confidence.expect("confidence").get();
        assert!((0.0..=1.0).contains(&conf), "confidence in [0,1]: {conf}");
        assert!(first.span.is_some(), "span should be populated");
    }

    #[test]
    fn empty_input_returns_empty_vec() {
        let det = english_only();
        assert!(det.detect("").unwrap().is_empty());
    }

    #[test]
    fn rejects_construction_with_no_recognised_languages() {
        let tags = ["xx".parse().unwrap()];
        assert!(LinguaLanguageDetector::for_languages(&tags).is_none());
    }

    #[test]
    fn for_all_languages_constructs() {
        let det = LinguaLanguageDetector::for_all_languages();
        let detections = det.detect("Hello world, this is English.").unwrap();
        assert!(!detections.is_empty());
    }

    #[test]
    fn detect_returns_span_offsets() {
        let det = english_only();
        let detections = det
            .detect("The quick brown fox jumps over the lazy dog.")
            .unwrap();
        let span = detections[0].span.expect("span");
        assert_eq!(span.start, 0);
        assert!(span.end > 0);
    }
}
