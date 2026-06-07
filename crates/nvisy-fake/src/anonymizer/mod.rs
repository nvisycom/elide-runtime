//! [`Fake`]: text-modality [`Anonymizer`] that swaps detected
//! entities for plausible fake values.

use async_trait::async_trait;
use fake::rand::SeedableRng;
use fake::rand::rngs::StdRng;
use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Text, TextData};
use nvisy_core::redaction::{Anonymizer, LeakProfile, TextReplacement};

use crate::generator;
use crate::locale::Locale;

/// Text-modality fake-data anonymizer.
///
/// Picks a locale from the entity's BCP-47 `language` field; entities
/// with no language tag fall back to English. RNG state is derived
/// per-call from the entity's UUID so repeat runs over the same
/// input produce the same fake value; pass a `seed` to add a
/// workspace-wide salt.
#[derive(Debug, Clone, Default)]
pub struct Fake {
    seed: u64,
}

impl Fake {
    /// Build a `Fake` operator with no salt.
    pub fn new() -> Self {
        Self::default()
    }

    /// Salt the per-call RNG with `seed`. Two operators with the same
    /// seed produce the same fake value for the same entity id.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    fn locale_for(&self, entity: &Entity<Text>) -> Locale {
        entity
            .language
            .as_ref()
            .map(Locale::from_tag)
            .unwrap_or_default()
    }

    fn rng_for(&self, entity: &Entity<Text>) -> StdRng {
        let (high, low) = entity.id.as_u64_pair();
        let seed = high ^ low ^ self.seed;
        StdRng::seed_from_u64(seed)
    }
}

#[async_trait]
impl Anonymizer<Text> for Fake {
    fn leak_profile(&self) -> LeakProfile {
        // The original value is gone; only the entity's position and
        // approximate shape (length differs from the original) leak.
        LeakProfile::Partial
    }

    async fn apply(&self, entity: &Entity<Text>, _source: &TextData) -> Result<TextReplacement> {
        let locale = self.locale_for(entity);
        let mut rng = self.rng_for(entity);
        let value = generator::generate(locale, entity.entity_kind, &mut rng)
            .unwrap_or_else(|| format!("[{}]", entity.entity_kind));
        Ok(TextReplacement::substituted(value))
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::entity::EntityKind;

    use super::*;

    fn text_entity(kind: EntityKind) -> Entity<Text> {
        Entity::test_builder(0, 4)
            .with_entity_kind(kind)
            .test_build()
    }

    #[tokio::test]
    async fn falls_back_to_placeholder_for_unsupported_kind() {
        let op = Fake::new();
        let entity = text_entity(EntityKind::IpAddress);
        let out = op.apply(&entity, &TextData::new("1234")).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("[ip_address]"));
    }

    #[tokio::test]
    async fn same_seed_and_entity_id_produces_same_output() {
        let op = Fake::new().with_seed(42);
        let entity = text_entity(EntityKind::PersonName);
        let a = op.apply(&entity, &TextData::new("alice")).await.unwrap();
        let b = op.apply(&entity, &TextData::new("alice")).await.unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn entity_language_picks_locale() {
        let op = Fake::new();
        let mut entity = text_entity(EntityKind::PersonName);
        entity.language = Some("ja".parse().unwrap());
        let out = op.apply(&entity, &TextData::new("name")).await.unwrap();
        let TextReplacement::Substituted { value } = out else {
            panic!("expected Substituted variant");
        };
        assert!(!value.is_empty(), "ja name should not be empty");
    }
}
