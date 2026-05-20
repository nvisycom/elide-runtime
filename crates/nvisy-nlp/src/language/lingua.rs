//! [`LinguaLanguageDetector`] — wraps the `lingua` crate's
//! [`lingua::LanguageDetector`].
//!
//! Constructors mirror [`lingua::LanguageDetectorBuilder`]:
//!
//! - [`for_languages`](LinguaLanguageDetector::for_languages) — restrict
//!   to a known set. **Preferred when the corpus language is known**;
//!   lingua is more accurate when the candidate set is narrowed.
//! - [`for_all_languages`](LinguaLanguageDetector::for_all_languages) —
//!   consider every language compiled into the `lingua` crate's
//!   feature set. Use only when the language is genuinely unknown.
//!
//! Neither constructor enables `low_accuracy_mode`; lingua's default
//! (highest accuracy) is in effect.

use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::{fmt, str::FromStr};

use lingua::{IsoCode639_1, Language, LanguageDetector as LinguaDetector, LanguageDetectorBuilder};
use nvisy_ontology::primitive::LanguageTag;

use super::{LanguageDetection, LanguageDetector, LanguageProvenance, LanguageSpan};
use crate::error::Result;

/// A [`LanguageDetector`] backed by [`lingua`].
///
/// [`lingua`]: https://crates.io/crates/lingua
pub struct LinguaLanguageDetector {
    inner: LinguaDetector,
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
        Some(Self { inner })
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
    fn detect(&self, text: &str) -> Result<Option<LanguageDetection>> {
        let Some(lang) = self.inner.detect_language_of(text) else {
            return Ok(None);
        };
        let Some(language) = Self::lingua_to_tag(lang) else {
            return Ok(None);
        };
        let confidence = self.inner.compute_language_confidence(text, lang);
        Ok(Some(LanguageDetection {
            language,
            confidence: Some(confidence),
            provenance: LanguageProvenance::Detected,
        }))
    }

    fn detect_multiple(&self, text: &str) -> Result<Vec<LanguageSpan>> {
        Ok(self
            .inner
            .detect_multiple_languages_of(text)
            .into_iter()
            .filter_map(|result| {
                let language = Self::lingua_to_tag(result.language())?;
                let confidence = self
                    .inner
                    .compute_language_confidence(text, result.language());
                Some(LanguageSpan {
                    start: result.start_index(),
                    end: result.end_index(),
                    language,
                    confidence: Some(confidence),
                })
            })
            .collect())
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
        let detection = det
            .detect("The quick brown fox jumps over the lazy dog.")
            .unwrap()
            .expect("detection");
        assert_eq!(detection.language.primary_language(), "en");
        let conf = detection.confidence.expect("confidence");
        assert!((0.0..=1.0).contains(&conf), "confidence in [0,1]: {conf}");
    }

    #[test]
    fn empty_input_returns_none() {
        let det = english_only();
        assert!(det.detect("").unwrap().is_none());
    }

    #[test]
    fn rejects_construction_with_no_recognised_languages() {
        let tags = ["xx".parse().unwrap()];
        assert!(LinguaLanguageDetector::for_languages(&tags).is_none());
    }

    #[test]
    fn for_all_languages_constructs() {
        let det = LinguaLanguageDetector::for_all_languages();
        let detection = det.detect("Hello world, this is English.").unwrap();
        assert!(detection.is_some());
    }

    #[test]
    fn detect_multiple_returns_at_least_one_span() {
        let det = english_only();
        let spans = det
            .detect_multiple("The quick brown fox jumps over the lazy dog.")
            .unwrap();
        assert!(!spans.is_empty());
        let first = &spans[0];
        assert_eq!(first.start, 0);
        assert!(first.end > 0);
    }
}
