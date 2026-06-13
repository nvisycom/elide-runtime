//! [`Mask`]: replace characters of the matched value with a fixed
//! mask character, optionally leaving a prefix and/or suffix of
//! the original value unmasked.
//!
//! Configurable along three axes:
//!
//! - `mask_char` — the character substituted in (default `'*'`).
//! - `keep_prefix` — number of leading characters to leave visible
//!   (default `0`). Use [`Mask::with_keep_prefix`].
//! - `keep_suffix` — number of trailing characters to leave
//!   visible (default `0`). Use [`Mask::with_keep_suffix`].
//!
//! Counts are character-based, not byte-based. When the combined
//! `keep_prefix + keep_suffix` exceeds the value's length the
//! whole value passes through unmasked. The output length always
//! equals the input length.
//!
//! Common patterns:
//!
//! - Mask everything: `Mask::stars()`.
//! - Show last 4 of a card: `Mask::stars().with_keep_suffix(4)`.
//! - Show BIN and last 4: `Mask::stars().with_keep_prefix(4).with_keep_suffix(4)`.

use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Tabular, Text, TextData};

use crate::redaction::{Anonymizer, LeakProfile, TabularReplacement, TextReplacement};

/// Character-replacement masking operator.
#[derive(Debug, Clone, Copy)]
pub struct Mask {
    mask_char: char,
    keep_prefix: usize,
    keep_suffix: usize,
}

impl Mask {
    /// Build a `Mask` operator with the given mask character and
    /// no preserved prefix / suffix — i.e. mask the whole value.
    pub fn new(mask_char: char) -> Self {
        Self {
            mask_char,
            keep_prefix: 0,
            keep_suffix: 0,
        }
    }

    /// Mask every character with `'*'`.
    pub fn stars() -> Self {
        Self::new('*')
    }

    /// Leave the first `n` characters of the value unmasked. Useful
    /// for "show BIN" card-display patterns.
    #[must_use]
    pub fn with_keep_prefix(mut self, n: usize) -> Self {
        self.keep_prefix = n;
        self
    }

    /// Leave the last `n` characters of the value unmasked. Useful
    /// for "show last 4" card-display patterns.
    #[must_use]
    pub fn with_keep_suffix(mut self, n: usize) -> Self {
        self.keep_suffix = n;
        self
    }

    /// Render `value` under this mask: keep `keep_prefix` leading
    /// and `keep_suffix` trailing characters verbatim, replace the
    /// rest with `mask_char`. When the kept regions cover (or
    /// overlap) the whole value the input is returned unchanged.
    /// Character-based, not byte-based, so multi-byte codepoints
    /// stay intact.
    pub fn render(&self, value: &str) -> String {
        let total = value.chars().count();
        // Clamp so prefix + suffix never overlap: when they sum past
        // `total`, the whole value passes through unmasked.
        if self.keep_prefix.saturating_add(self.keep_suffix) >= total {
            return value.to_string();
        }
        let suffix_start = total - self.keep_suffix;
        value
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i < self.keep_prefix || i >= suffix_start {
                    c
                } else {
                    self.mask_char
                }
            })
            .collect()
    }
}

impl Default for Mask {
    fn default() -> Self {
        Self::stars()
    }
}

#[async_trait::async_trait]
impl Anonymizer<Text> for Mask {
    fn leak_profile(&self) -> LeakProfile {
        LeakProfile::Partial
    }

    async fn apply(&self, _entity: &Entity<Text>, source: &TextData) -> Result<TextReplacement> {
        Ok(TextReplacement::substituted(
            self.render(source.text.as_str()),
        ))
    }
}

#[async_trait::async_trait]
impl Anonymizer<Tabular> for Mask {
    fn leak_profile(&self) -> LeakProfile {
        LeakProfile::Partial
    }

    async fn apply(
        &self,
        _entity: &Entity<Tabular>,
        source: &TextData,
    ) -> Result<TabularReplacement> {
        Ok(TabularReplacement::substituted(
            self.render(source.text.as_str()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::entity::{TrailStep, builtins};
    use nvisy_core::modality::TextLocation;
    use nvisy_core::primitive::Confidence;

    use super::*;

    fn entity(start: usize, end: usize) -> Entity<Text> {
        Entity::builder()
            .with_label(builtins::PAYMENT_CARD.label_ref())
            .with_location(TextLocation::new(start, end))
            .with_confidence(Confidence::new(1.0).unwrap())
            .with_trail(Vec::<TrailStep>::new())
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn mask_all_with_stars() {
        let op = Mask::stars();
        let source = TextData::new("4111111111111111");
        let entity = entity(0, source.text.len());
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("****************"));
    }

    #[tokio::test]
    async fn keep_suffix_shows_last_n() {
        let op = Mask::new('#').with_keep_suffix(4);
        let source = TextData::new("4111111111111111");
        let entity = entity(0, source.text.len());
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("############1111"));
    }

    #[tokio::test]
    async fn keep_prefix_shows_first_n() {
        let op = Mask::new('#').with_keep_prefix(4);
        let source = TextData::new("4111111111111111");
        let entity = entity(0, source.text.len());
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("4111############"));
    }

    #[tokio::test]
    async fn keep_prefix_and_suffix_show_both_ends() {
        let op = Mask::new('*').with_keep_prefix(4).with_keep_suffix(4);
        let source = TextData::new("4111111111111111");
        let entity = entity(0, source.text.len());
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("4111********1111"));
    }

    #[tokio::test]
    async fn keep_exceeding_length_passes_value_through() {
        // 16-char value with prefix+suffix asking for 20 visible
        // chars: nothing left to mask, return verbatim.
        let op = Mask::new('*').with_keep_prefix(10).with_keep_suffix(10);
        let source = TextData::new("4111111111111111");
        let entity = entity(0, source.text.len());
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("4111111111111111"));
    }
}
