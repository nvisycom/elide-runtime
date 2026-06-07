//! [`Fake`]: text-modality [`Anonymizer`] that swaps detected
//! entities for plausible fake values.

use std::hash::{DefaultHasher, Hash, Hasher};

use fake::rand::SeedableRng;
use fake::rand::rngs::SmallRng;
use nvisy_core::Result;
use nvisy_core::entity::{Entity, EntityKind};
use nvisy_core::modality::{Text, TextData};
use nvisy_core::primitive::LanguageTag;
use nvisy_core::redaction::{Anonymizer, LeakProfile, TextReplacement};

use crate::generator;
use crate::locale::Locale;

/// Text-modality fake-data anonymizer.
///
/// Picks a locale from the entity's BCP-47 `language` field, falling
/// back to the `default_language` passed at construction when the
/// entity carries no tag. RNG state is derived per-call from the
/// entity's coreference id (or its UUID when there is none), so
/// coreferent mentions of the same real-world entity collapse to the
/// same fake value within a run. Pass a `seed` to add a
/// workspace-wide salt.
#[derive(Debug, Clone)]
pub struct Fake {
    default_language: LanguageTag,
    seed: u64,
    length_preserving: bool,
    format_preserving: bool,
}

impl Fake {
    /// Build a `Fake` operator that uses `default_language` for
    /// entities with no language tag.
    pub fn new(default_language: LanguageTag) -> Self {
        Self {
            default_language,
            seed: 0,
            length_preserving: false,
            format_preserving: false,
        }
    }

    /// Salt the per-call RNG with `seed`. Two operators with the same
    /// seed produce the same fake value for the same entity id.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Clip / right-pad the fake value to match the original entity
    /// span length. Only applies to fixed-width kinds
    /// ([`EntityKind::PaymentCard`], [`EntityKind::Iban`],
    /// [`EntityKind::PostalCode`]); free-form kinds (names,
    /// addresses) ignore the flag.
    #[must_use]
    pub fn length_preserving(mut self) -> Self {
        self.length_preserving = true;
        self
    }

    /// Preserve non-digit separators (spaces, dashes, dots) found in
    /// the original span when emitting fakes for digit-shaped kinds
    /// like [`EntityKind::PhoneNumber`] and
    /// [`EntityKind::PostalCode`]. Free-form kinds ignore the flag.
    #[must_use]
    pub fn format_preserving(mut self) -> Self {
        self.format_preserving = true;
        self
    }

    pub(crate) fn locale_for(&self, entity: &Entity<Text>) -> Locale {
        let tag = entity.language.as_ref().unwrap_or(&self.default_language);
        Locale::from_tag(tag)
    }

    pub(crate) fn rng_for(&self, entity: &Entity<Text>) -> SmallRng {
        SmallRng::seed_from_u64(self.entity_seed(entity))
    }

    /// Stable per-entity seed combining the workspace salt and a
    /// coreference-aware identity: prefer `entity_id` (shared by
    /// coreferent mentions) and fall back to the entity's UUID.
    fn entity_seed(&self, entity: &Entity<Text>) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.seed.hash(&mut hasher);
        match entity.entity_id.as_deref() {
            Some(id) => id.hash(&mut hasher),
            None => entity.id.as_bytes().hash(&mut hasher),
        }
        hasher.finish()
    }
}

impl Default for Fake {
    /// English (`"en"`) as the default language, with no RNG salt.
    fn default() -> Self {
        Self::new("en".parse().expect("\"en\" is a valid BCP-47 tag"))
    }
}

#[async_trait::async_trait]
impl Anonymizer<Text> for Fake {
    fn leak_profile(&self) -> LeakProfile {
        // The original value is gone; only the entity's position and
        // approximate shape (length differs from the original) leak.
        LeakProfile::Partial
    }

    async fn apply(&self, entity: &Entity<Text>, source: &TextData) -> Result<TextReplacement> {
        let locale = self.locale_for(entity);
        let mut rng = self.rng_for(entity);
        let original = read_span(entity, source);
        let value = generator::generate(
            generator::Context {
                locale,
                kind: entity.entity_kind,
                length_preserving: self.length_preserving,
                format_preserving: self.format_preserving,
                original,
            },
            &mut rng,
        )
        .unwrap_or_else(|| format!("[{}]", entity.entity_kind));
        Ok(TextReplacement::substituted(value))
    }
}

/// Borrow the substring at `entity.location` from `source`. Returns
/// `""` for empty / out-of-bounds / mid-char ranges.
fn read_span<'a>(entity: &Entity<Text>, source: &'a TextData) -> &'a str {
    let text = source.text.as_str();
    let start = entity.location.start.min(text.len());
    let end = entity.location.end.min(text.len());
    if start >= end || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
        return "";
    }
    &text[start..end]
}

/// Marker for kinds that honor [`Fake::length_preserving`].
pub(crate) fn is_fixed_width(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::PaymentCard | EntityKind::Iban | EntityKind::PostalCode
    )
}

/// Marker for kinds that honor [`Fake::format_preserving`].
pub(crate) fn honors_format(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::PhoneNumber | EntityKind::PostalCode)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn text_entity(kind: EntityKind) -> Entity<Text> {
        Entity::test_builder(0, 4)
            .with_entity_kind(kind)
            .test_build()
    }

    #[tokio::test]
    async fn falls_back_to_placeholder_for_unsupported_kind() {
        let op = Fake::default();
        let entity = text_entity(EntityKind::IpAddress);
        let out = op.apply(&entity, &TextData::new("1234")).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("[ip_address]"));
    }

    #[tokio::test]
    async fn same_seed_and_entity_id_produces_same_output() {
        let op = Fake::default().with_seed(42);
        let entity = text_entity(EntityKind::PersonName);
        let a = op.apply(&entity, &TextData::new("alice")).await.unwrap();
        let b = op.apply(&entity, &TextData::new("alice")).await.unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn entity_language_overrides_default() {
        let op = Fake::default();
        let mut entity = text_entity(EntityKind::PersonName);
        entity.language = Some("ja".parse().unwrap());
        let out = op.apply(&entity, &TextData::new("name")).await.unwrap();
        let TextReplacement::Substituted { value } = out else {
            panic!("expected Substituted variant");
        };
        assert!(!value.is_empty(), "ja name should not be empty");
    }

    #[tokio::test]
    async fn default_language_applies_when_entity_unlanguaged() {
        let op = Fake::new("ja".parse().unwrap());
        let entity = text_entity(EntityKind::PersonName);
        let out = op.apply(&entity, &TextData::new("name")).await.unwrap();
        let TextReplacement::Substituted { value } = out else {
            panic!("expected Substituted variant");
        };
        assert!(!value.is_empty());
    }

    #[tokio::test]
    async fn coreferent_entities_collapse_to_same_fake() {
        let op = Fake::default();
        let a = Entity::test_builder(0, 4)
            .with_entity_kind(EntityKind::PersonName)
            .with_entity_id("ENTITY_42".to_string())
            .test_build();
        let b = Entity::test_builder(10, 14)
            .with_entity_kind(EntityKind::PersonName)
            .with_entity_id("ENTITY_42".to_string())
            .test_build();
        let out_a = op.apply(&a, &TextData::new("aliceb")).await.unwrap();
        let out_b = op.apply(&b, &TextData::new("aliceb")).await.unwrap();
        assert_eq!(out_a, out_b);
    }

    #[tokio::test]
    async fn distinct_entities_get_distinct_fakes() {
        let op = Fake::default();
        let mut outputs: HashSet<String> = HashSet::new();
        for _ in 0..32 {
            let entity = text_entity(EntityKind::PersonName);
            let out = op.apply(&entity, &TextData::new("seed")).await.unwrap();
            let TextReplacement::Substituted { value } = out else {
                panic!("expected Substituted");
            };
            outputs.insert(value);
        }
        assert!(
            outputs.len() >= 30,
            "expected >=30 distinct fakes across 32 fresh entity ids, got {}",
            outputs.len(),
        );
    }
}
