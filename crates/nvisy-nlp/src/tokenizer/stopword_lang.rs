//! Allowlist of ISO 639-1 codes the [`stop-words`] crate recognises
//! under the `iso` feature flag we enable in the workspace.
//!
//! Used to gate calls to [`stop_words::get`] so we never reach the
//! crate's panic path on unknown codes.
//!
//! Keep this in sync with the `stop-words` crate's `LANGUAGE` enum.
//! Adding more languages here requires enabling additional features
//! on the workspace `stop-words` dependency.
//!
//! [`stop-words`]: https://crates.io/crates/stop-words

/// ISO 639-1 codes the `iso`-featured `stop-words` build recognises.
const SUPPORTED: &[&str] = &[
    "ar", "da", "nl", "en", "fi", "fr", "de", "el", "hu", "id", "it", "no", "pt", "ro", "ru", "sl",
    "es", "sv", "tr",
];

/// Whether `code` (BCP-47 primary subtag / ISO 639-1) has a stopword
/// list available without panicking the `stop-words` crate.
pub(crate) fn is_supported(code: &str) -> bool {
    SUPPORTED.contains(&code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_supported() {
        assert!(is_supported("en"));
    }

    #[test]
    fn nonsense_codes_are_not() {
        assert!(!is_supported("xx"));
        assert!(!is_supported(""));
        assert!(!is_supported("EN"));
    }
}
