//! Lingua-backed language detection: [`LinguaLanguagePolicy`] (the
//! factory) and [`LinguaLanguageDetector`] (the produced detector).
//!
//! The detector struct is public so [`LinguaLanguagePolicy`] can
//! name it as its associated detector type, but the constructors
//! are crate-private: external code reaches Lingua-backed detection
//! through [`LinguaLanguagePolicy`], not by building detectors
//! directly. This keeps the "policy is the factory" model honest.
//!
//! [`lingua`]: https://crates.io/crates/lingua

use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};

use lingua::{IsoCode639_1, Language, LanguageDetector as LinguaDetector, LanguageDetectorBuilder};
use nvisy_ontology::primitive::{Confidence, LanguageTag};

use super::{
    LanguageDetection, LanguageDetector, LanguagePolicy, LanguageProvenance, LanguageSpan,
};
use crate::error::Result;

/// Language detector backed by the [`lingua`] crate.
///
/// Constructed via [`LinguaLanguagePolicy`]; the constructors on
/// this type are crate-private. The struct is public so the policy
/// can name it as its associated `Detector` type, but external code
/// can't invoke it directly — call through the policy or the
/// engine.
///
/// [`lingua`]: https://crates.io/crates/lingua
pub struct LinguaLanguageDetector {
    inner: LinguaDetector,
}

impl LinguaLanguageDetector {
    /// Construct a detector restricted to the given languages.
    ///
    /// Unrecognised tags (no matching ISO 639-1 primary subtag
    /// among lingua's supported languages) are silently skipped.
    /// Returns `None` when no input tag is recognised — a detector
    /// with zero candidates is useless.
    pub(crate) fn for_languages(tags: &[LanguageTag]) -> Option<Self> {
        let langs = tags_to_languages(tags);
        if langs.is_empty() {
            return None;
        }
        let inner = LanguageDetectorBuilder::from_languages(&langs).build();
        Some(Self { inner })
    }

    /// Construct a detector considering every language compiled
    /// into the `lingua` crate's feature set.
    pub(crate) fn for_all_languages() -> Self {
        let inner = LanguageDetectorBuilder::from_all_languages().build();
        Self { inner }
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
        let detections = self
            .inner
            .detect_multiple_languages_of(text)
            .into_iter()
            .filter_map(|result| {
                let language = Self::lingua_to_tag(result.language())?;
                let raw_confidence = self
                    .inner
                    .compute_language_confidence(text, result.language());
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

impl fmt::Debug for LinguaLanguageDetector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinguaLanguageDetector").finish()
    }
}

/// [`LanguagePolicy`] backed by the [`lingua`] crate.
///
/// Unit-like: holds no state. Every call builds a fresh
/// [`LinguaLanguageDetector`] restricted to the requested language
/// set (or considering everything `lingua` is feature-enabled for
/// when no set is given).
///
/// Construction of the underlying lingua detector is on the order
/// of milliseconds; if the caller's hot path can't afford that,
/// wrap the policy in your own cache rather than asking this type
/// to grow state.
///
/// [`lingua`]: https://crates.io/crates/lingua
#[derive(Debug, Default, Clone, Copy)]
pub struct LinguaLanguagePolicy;

impl LanguagePolicy for LinguaLanguagePolicy {
    type Detector = LinguaLanguageDetector;

    fn detector_for_all(&self) -> LinguaLanguageDetector {
        LinguaLanguageDetector::for_all_languages()
    }

    fn detector_for(&self, languages: &[LanguageTag]) -> LinguaLanguageDetector {
        if languages.is_empty() {
            return self.detector_for_all();
        }
        // Unrecognised tags are silently skipped; if the result is
        // empty, fall back to the unrestricted detector rather than
        // returning a useless zero-language one.
        LinguaLanguageDetector::for_languages(languages).unwrap_or_else(|| self.detector_for_all())
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
