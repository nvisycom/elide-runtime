//! [`Token`] and the [`Tokens`] collection.
//!
//! A [`Token`] is the engine's view of one lexical unit: the
//! surface text, its lemma when the engine has a lemmatizer
//! (otherwise == `text`), the byte range it occupies in the source
//! text, and two precomputed predicates the enhancer reads
//! (`is_stop`, `is_punct`).
//!
//! [`Tokens`] is the owning collection plus lookup helpers the
//! enhancer uses: [`around`] gets the slice of
//! tokens within a byte window, [`lemmas_in`]
//! iterates lemmas covering a byte range. Both work in *source-text
//! byte offsets* — the same coordinate space as
//! [`Entity::location`]
//! — so there's no coordinate translation at the call site.
//!
//! [`around`]: Tokens::around
//! [`lemmas_in`]: Tokens::lemmas_in
//! [`Entity::location`]: crate::entity::Entity::location
//!
//! The shape is intentionally minimal. POS tags, morphology,
//! dependency trees, and other heavier features are not part of the
//! v1 surface; they get added as fields when a downstream consumer
//! needs them. This keeps the artifact cheap for engines that don't
//! produce them — `text == lemma`, `is_stop == false`,
//! `is_punct == false` are the defaults for a tokenizer-only
//! engine.

use std::ops::Range;

use hipstr::HipStr;

/// One token produced by an `NlpEngine`.
///
/// `lemma` falls back to `text` when the engine has no lemmatizer —
/// callers that want lemma-aware matching can read `token.lemma`
/// uniformly without checking which engine produced the artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Surface form as it appears in the source text.
    pub text: HipStr<'static>,
    /// Lemma when the engine produced one; otherwise == [`text`].
    ///
    /// [`text`]: Self::text
    pub lemma: HipStr<'static>,
    /// Byte range this token occupies in the source text. Use this
    /// to map back to substrings of the original input.
    pub offset: Range<usize>,
    /// Engine-asserted stopword flag (e.g. "the", "a", "of" for
    /// English). Engines without a stopword list set this to
    /// `false`; the artifact's [`StopwordSet`]
    /// is the authoritative source.
    ///
    /// [`StopwordSet`]: super::StopwordSet
    pub is_stop: bool,
    /// Engine-asserted punctuation flag.
    pub is_punct: bool,
}

impl Token {
    /// Construct a token with the surface form mirrored into
    /// `lemma`. Use this from tokenizer-only engines.
    pub fn from_text(text: impl Into<HipStr<'static>>, offset: Range<usize>) -> Self {
        let text = text.into();
        Self {
            lemma: text.clone(),
            text,
            offset,
            is_stop: false,
            is_punct: false,
        }
    }

    /// Override the lemma.
    #[must_use]
    pub fn with_lemma(mut self, lemma: impl Into<HipStr<'static>>) -> Self {
        self.lemma = lemma.into();
        self
    }

    /// Override the stopword flag.
    #[must_use]
    pub fn with_is_stop(mut self, is_stop: bool) -> Self {
        self.is_stop = is_stop;
        self
    }

    /// Override the punctuation flag.
    #[must_use]
    pub fn with_is_punct(mut self, is_punct: bool) -> Self {
        self.is_punct = is_punct;
        self
    }
}

/// The owning token sequence carried by a
/// [`RecognizerInput::artifacts`] bundle.
///
/// [`RecognizerInput::artifacts`]: crate::RecognizerInput::artifacts
///
/// Tokens are sorted by `offset.start` (engines should produce them
/// in order; consumer-side code assumes this). The collection
/// exposes byte-range lookup helpers the `ContextEnhancer` uses to
/// pull lemmas around an entity match.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tokens(Vec<Token>);

impl Tokens {
    /// Construct an empty token sequence. Use this when the engine
    /// has no tokenizer (language-only engines).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct from an owned vector. The caller is responsible
    /// for ensuring tokens are sorted by `offset.start`.
    #[must_use]
    pub fn new(tokens: Vec<Token>) -> Self {
        Self(tokens)
    }

    /// Borrow the underlying slice.
    #[must_use]
    pub fn as_slice(&self) -> &[Token] {
        &self.0
    }

    /// Number of tokens.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the sequence is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate tokens in source order.
    pub fn iter(&self) -> std::slice::Iter<'_, Token> {
        self.0.iter()
    }

    /// Tokens overlapping `byte_range`, plus a `window`-byte
    /// margin on each side.
    ///
    /// Used by the enhancer to grab the keyword neighborhood around
    /// an entity match. Returns the contiguous sub-slice; tokens at
    /// the boundary are included when their byte range overlaps the
    /// expanded range.
    ///
    /// Cost is `O(log n)` for the start probe + linear over the
    /// returned slice; the sequence is sorted so a binary search
    /// suffices.
    #[must_use]
    pub fn around(&self, byte_range: Range<usize>, window: usize) -> &[Token] {
        let lo = byte_range.start.saturating_sub(window);
        let hi = byte_range.end.saturating_add(window);
        self.in_range(lo..hi)
    }

    /// Lemmas of every token overlapping `byte_range`. Useful when
    /// only the lemma strings are needed (e.g. for keyword
    /// matching).
    pub fn lemmas_in(&self, byte_range: Range<usize>) -> impl Iterator<Item = &str> {
        self.in_range(byte_range).iter().map(|t| t.lemma.as_str())
    }

    /// Tokens fully contained within (or overlapping) `byte_range`.
    /// Returned as a sub-slice — tokens with `offset.end > range.start`
    /// and `offset.start < range.end` are included.
    #[must_use]
    pub fn in_range(&self, byte_range: Range<usize>) -> &[Token] {
        if self.0.is_empty() || byte_range.start >= byte_range.end {
            return &[];
        }
        let start = self.0.partition_point(|t| t.offset.end <= byte_range.start);
        let end = self.0.partition_point(|t| t.offset.start < byte_range.end);
        if start >= end {
            return &[];
        }
        &self.0[start..end]
    }
}

impl FromIterator<Token> for Tokens {
    fn from_iter<I: IntoIterator<Item = Token>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoIterator for Tokens {
    type IntoIter = std::vec::IntoIter<Token>;
    type Item = Token;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(text: &'static str, start: usize, end: usize) -> Token {
        Token::from_text(text, start..end)
    }

    #[test]
    fn in_range_returns_overlapping_tokens() {
        let tokens = Tokens::new(vec![t("hello", 0, 5), t("world", 6, 11), t("foo", 12, 15)]);
        // 4..7 overlaps "hello" and "world"
        let got: Vec<&str> = tokens
            .in_range(4..7)
            .iter()
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(got, vec!["hello", "world"]);
    }

    #[test]
    fn around_extends_by_window() {
        let tokens = Tokens::new(vec![
            t("a", 0, 1),
            t("b", 2, 3),
            t("c", 4, 5),
            t("d", 6, 7),
            t("e", 8, 9),
        ]);
        // around 4..5 with window=2 → look at 2..7 → "b","c","d"
        let got: Vec<&str> = tokens
            .around(4..5, 2)
            .iter()
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(got, vec!["b", "c", "d"]);
    }

    #[test]
    fn lemmas_in_yields_lemmas() {
        let tokens = Tokens::new(vec![
            t("running", 0, 7).with_lemma("run"),
            t("dogs", 8, 12).with_lemma("dog"),
        ]);
        let got: Vec<&str> = tokens.lemmas_in(0..12).collect();
        assert_eq!(got, vec!["run", "dog"]);
    }

    #[test]
    fn in_range_empty_for_disjoint_range() {
        let tokens = Tokens::new(vec![t("a", 0, 5)]);
        assert!(tokens.in_range(10..20).is_empty());
    }

    #[test]
    fn in_range_empty_for_inverted_range() {
        let tokens = Tokens::new(vec![t("a", 0, 5)]);
        let inverted = Range {
            start: 5usize,
            end: 3usize,
        };
        assert!(tokens.in_range(inverted).is_empty());
    }
}
