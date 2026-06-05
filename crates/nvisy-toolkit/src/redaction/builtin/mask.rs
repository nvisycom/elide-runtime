//! [`Mask`]: replace characters of the matched value with a fixed
//! mask character.
//!
//! Configurable along three axes:
//!
//! - `mask_char` — the character substituted in (default `'*'`).
//! - `chars_to_mask` — how many characters to mask. `None` masks
//!   every character of the value.
//! - `from_end` — when `true`, masking starts from the end of the
//!   value rather than the beginning (e.g. last 4 of a credit
//!   card number).
//!
//! The unmasked tail/prefix of the value is preserved literally;
//! the output length always equals the original value length.

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Text, TextData};

use super::text_value::read_value;
use crate::redaction::{Anonymizer, LeakProfile, TextReplacement};

/// Character-replacement masking operator.
#[derive(Debug, Clone, Copy)]
pub struct Mask {
    mask_char: char,
    chars_to_mask: Option<usize>,
    from_end: bool,
}

impl Mask {
    /// Build a `Mask` operator with the given character and a
    /// per-call cap on how many characters to mask. `None` masks the
    /// whole value.
    pub fn new(mask_char: char, chars_to_mask: Option<usize>) -> Self {
        Self {
            mask_char,
            chars_to_mask,
            from_end: false,
        }
    }

    /// Convenience constructor matching the common "mask the whole
    /// value with `*`" pattern.
    pub fn stars() -> Self {
        Self::new('*', None)
    }

    /// Mask from the end of the value instead of the beginning. Useful
    /// for "show only the last 4 digits" patterns when paired with a
    /// `chars_to_mask` that leaves the tail visible.
    #[must_use]
    pub fn from_end(mut self) -> Self {
        self.from_end = true;
        self
    }
}

impl Default for Mask {
    fn default() -> Self {
        Self::stars()
    }
}

#[async_trait]
impl Anonymizer<Text> for Mask {
    fn leak_profile(&self) -> LeakProfile {
        LeakProfile::Partial
    }

    async fn apply(&self, entity: &Entity<Text>, source: &TextData) -> Result<TextReplacement> {
        let value = read_value(entity, source);
        Ok(TextReplacement::substituted(mask(
            value,
            self.mask_char,
            self.chars_to_mask,
            self.from_end,
        )))
    }
}

fn mask(value: &str, mask_char: char, chars_to_mask: Option<usize>, from_end: bool) -> String {
    let total = value.chars().count();
    let to_mask = chars_to_mask.map(|n| n.min(total)).unwrap_or(total);

    if from_end {
        let keep = total - to_mask;
        value
            .chars()
            .enumerate()
            .map(|(i, c)| if i < keep { c } else { mask_char })
            .collect()
    } else {
        value
            .chars()
            .enumerate()
            .map(|(i, c)| if i < to_mask { mask_char } else { c })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::entity::{EntityKind, TrailStep};
    use nvisy_core::modality::TextLocation;
    use nvisy_core::primitive::Confidence;

    use super::*;

    fn entity(start: usize, end: usize) -> Entity<Text> {
        Entity::builder()
            .with_entity_kind(EntityKind::PaymentCard)
            .with_location(TextLocation::new(start, end))
            .with_confidence(Confidence::new(1.0).unwrap())
            .with_trail(Vec::<TrailStep>::new())
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn mask_all_with_stars() {
        let op = Mask::stars();
        let source = TextData::new("card 4111111111111111 here");
        let entity = entity(5, 21);
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("****************"));
    }

    #[tokio::test]
    async fn mask_first_n_keeps_tail() {
        let op = Mask::new('#', Some(12));
        let source = TextData::new("card 4111111111111111 here");
        let entity = entity(5, 21);
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("############1111"));
    }

    #[tokio::test]
    async fn mask_from_end_keeps_prefix() {
        let op = Mask::new('#', Some(12)).from_end();
        let source = TextData::new("card 4111111111111111 here");
        let entity = entity(5, 21);
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("4111############"));
    }

    #[tokio::test]
    async fn chars_to_mask_capped_at_value_length() {
        let op = Mask::new('*', Some(999));
        let source = TextData::new("hi");
        let entity = entity(0, 2);
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("**"));
    }
}
