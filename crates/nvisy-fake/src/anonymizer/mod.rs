//! [`Fake`]: text-modality [`Anonymizer`] that swaps detected
//! entities for plausible fake values.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::str::FromStr;
use std::sync::Arc;

use fake::rand::SeedableRng;
use fake::rand::rngs::SmallRng;
use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Modality, Tabular, Text, TextData, TextLocation};
use nvisy_core::primitive::LanguageTag;
use nvisy_core::redaction::{Anonymizer, LeakProfile, TabularReplacement, TextReplacement};

use crate::generator;
use crate::locale::Locale;

/// Text-modality fake-data anonymizer.
///
/// Picks a locale from the entity's BCP-47 `language` field, falling
/// back to the `default_language` (English unless overridden) when
/// the entity carries no tag. RNG state is derived per-call from
/// the entity's coreference id (or its UUID when there is none),
/// so coreferent mentions of the same real-world entity collapse to
/// the same fake value within a run.
///
/// Entity kinds outside the core PII catalogue delegate to the
/// `fallback` anonymizer passed at construction. There is no
/// implicit default: the caller must say what should happen for
/// unsupported kinds.
///
/// Structured kinds (IBAN, payment card, postal code, phone,
/// date-of-birth, etc.) always pattern-preserve the original — the
/// output's length and character-class layout matches the input,
/// only the digits and letters are randomised. Free-form kinds
/// (names, addresses, organisations) emit a fresh locale-aware
/// fake whose length doesn't need to match.
#[derive(Clone)]
pub struct Fake {
    fallback: Arc<dyn Anonymizer<Text>>,
    default_language: LanguageTag,
    seed: u64,
}

impl std::fmt::Debug for Fake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fake")
            .field("default_language", &self.default_language)
            .field("seed", &self.seed)
            .field("fallback", &"Arc<dyn Anonymizer<Text>>")
            .finish()
    }
}

impl Fake {
    /// Build a `Fake` operator with `fallback` as the anonymizer
    /// used for entity kinds outside the core PII catalogue.
    pub fn new<A>(fallback: A) -> Self
    where
        A: Anonymizer<Text> + 'static,
    {
        Self {
            fallback: Arc::new(fallback),
            default_language: LanguageTag::from_str("en").expect("en is BCP-47"),
            seed: 0,
        }
    }

    /// Override the default language used when the entity carries
    /// no `language` tag. Initial value is `"en"`.
    #[must_use]
    pub fn with_default_language(mut self, language: LanguageTag) -> Self {
        self.default_language = language;
        self
    }

    /// Salt the per-call RNG with `seed`. Two operators with the
    /// same seed produce the same fake value for the same entity.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    fn locale_for(&self, language: Option<&LanguageTag>) -> Locale {
        Locale::from_tag(language.unwrap_or(&self.default_language))
    }

    fn rng_for(&self, identity: Identity<'_>) -> SmallRng {
        let mut hasher = DefaultHasher::new();
        self.seed.hash(&mut hasher);
        identity.hash(&mut hasher);
        SmallRng::seed_from_u64(hasher.finish())
    }

    /// Try the generator for `label`; return `None` if it has no
    /// entry, so the caller can delegate to the fallback.
    fn try_generate(
        &self,
        locale: Locale,
        label: &str,
        identity: Identity<'_>,
        source: &str,
    ) -> Option<String> {
        let mut rng = self.rng_for(identity);
        generator::Context::new(locale, label, source).generate(&mut rng)
    }
}

#[async_trait::async_trait]
impl Anonymizer<Text> for Fake {
    fn leak_profile(&self) -> LeakProfile {
        // The original value is gone; only the entity's position
        // and approximate shape (length differs from the original)
        // leak.
        LeakProfile::Partial
    }

    async fn apply(&self, entity: &Entity<Text>, source: &TextData) -> Result<TextReplacement> {
        let locale = self.locale_for(entity.language.as_ref());
        match self.try_generate(
            locale,
            entity.label.as_str(),
            Identity::from(entity),
            source.text.as_str(),
        ) {
            Some(value) => Ok(TextReplacement::substituted(value)),
            None => self.fallback.apply(entity, source).await,
        }
    }
}

#[async_trait::async_trait]
impl Anonymizer<Tabular> for Fake {
    fn leak_profile(&self) -> LeakProfile {
        LeakProfile::Partial
    }

    async fn apply(
        &self,
        entity: &Entity<Tabular>,
        source: &TextData,
    ) -> Result<TabularReplacement> {
        let locale = self.locale_for(entity.language.as_ref());
        if let Some(value) = self.try_generate(
            locale,
            entity.label.as_str(),
            Identity::from(entity),
            source.text.as_str(),
        ) {
            return Ok(TabularReplacement::substituted(value));
        }
        // No Tabular fallback — borrow the Text fallback by
        // adapting the synthetic Text entity. The replacement
        // values are both string-typed, so the adaptation is just
        // a variant rewrap.
        let text_entity = adapt_to_text(entity);
        match self.fallback.apply(&text_entity, source).await? {
            TextReplacement::Substituted { value } => Ok(TabularReplacement::substituted(value)),
            TextReplacement::Removed => Ok(TabularReplacement::ColumnDropped),
        }
    }
}

/// Adapt a `Entity<Tabular>` into an `Entity<Text>` for delegating
/// to a Text-shaped fallback. Only the identity-relevant fields
/// (id, entity_id, entity_kind, language) need to round-trip; the
/// location is a synthetic full-span TextLocation since the
/// fallback's `apply` body reads source directly.
fn adapt_to_text(entity: &Entity<Tabular>) -> Entity<Text> {
    let mut builder = Entity::<Text>::builder()
        .with_id(entity.id)
        .with_label(entity.label.clone())
        .with_location(TextLocation::new(0, 0))
        .with_confidence(entity.confidence)
        .with_trail(entity.trail.clone());
    if let Some(id) = entity.entity_id.clone() {
        builder = builder.with_entity_id(id);
    }
    if let Some(lang) = entity.language.clone() {
        builder = builder.with_language(lang);
    }
    builder.build().expect("text entity adaptation")
}

/// Identity key used to seed the rng. Prefers the coreference id
/// (shared across mentions of the same real-world entity) and falls
/// back to the entity's UUID bytes.
enum Identity<'a> {
    Coref(&'a str),
    Uuid([u8; 16]),
}

impl<'a, M> From<&'a Entity<M>> for Identity<'a>
where
    M: Modality,
{
    fn from(entity: &'a Entity<M>) -> Self {
        match entity.entity_id.as_deref() {
            Some(id) => Identity::Coref(id),
            None => Identity::Uuid(*entity.id.as_bytes()),
        }
    }
}

impl Hash for Identity<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Identity::Coref(s) => {
                0u8.hash(state);
                s.hash(state);
            }
            Identity::Uuid(bytes) => {
                1u8.hash(state);
                bytes.hash(state);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use nvisy_core::entity::{EntityLabelRef, builtins};
    use nvisy_core::redaction::Anonymizer as _;
    use nvisy_toolkit::redaction::anonymizer::{Mask, Replace};

    use super::*;

    fn fake() -> Fake {
        Fake::new(Mask::stars())
    }

    /// Build an entity whose location spans the entire `source`
    /// string. Tests don't care about offsets — making them match
    /// the source makes them self-documenting.
    fn entity_over(label: EntityLabelRef, source: &str) -> Entity<Text> {
        Entity::test_builder(0, source.len())
            .with_label(label)
            .test_build()
    }

    fn coref_entity(label: EntityLabelRef, source: &str, coref_id: &str) -> Entity<Text> {
        Entity::test_builder(0, source.len())
            .with_label(label)
            .with_entity_id(coref_id.to_string())
            .test_build()
    }

    #[tokio::test]
    async fn unsupported_kind_delegates_to_fallback() {
        // Diagnosis isn't faked — sensitive clinical kinds are
        // intentionally excluded — so it falls through to the
        // fallback anonymizer.
        let op = Fake::new(Replace::new("[redacted]"));
        let source = TextData::new("hypertension");
        let entity = entity_over(builtins::DIAGNOSIS.label_ref(), source.text.as_str());
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("[redacted]"));
    }

    #[tokio::test]
    async fn fallback_can_be_mask() {
        let op = Fake::new(Mask::stars());
        let source = TextData::new("hypertension");
        let entity = entity_over(builtins::DIAGNOSIS.label_ref(), source.text.as_str());
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("************"));
    }

    #[tokio::test]
    async fn same_seed_and_entity_id_produces_same_output() {
        let op = fake().with_seed(42);
        let source = TextData::new("alice");
        let entity = entity_over(builtins::PERSON_NAME.label_ref(), source.text.as_str());
        let a = op.apply(&entity, &source).await.unwrap();
        let b = op.apply(&entity, &source).await.unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn entity_language_overrides_default() {
        let op = fake();
        let source = TextData::new("名前");
        let mut entity = entity_over(builtins::PERSON_NAME.label_ref(), source.text.as_str());
        entity.language = Some("ja".parse().unwrap());
        let out = op.apply(&entity, &source).await.unwrap();
        let TextReplacement::Substituted { value } = out else {
            panic!("expected Substituted variant");
        };
        assert!(!value.is_empty(), "ja name should not be empty");
    }

    #[tokio::test]
    async fn default_language_applies_when_entity_unlanguaged() {
        let op = fake().with_default_language("ja".parse().unwrap());
        let source = TextData::new("name");
        let entity = entity_over(builtins::PERSON_NAME.label_ref(), source.text.as_str());
        let out = op.apply(&entity, &source).await.unwrap();
        let TextReplacement::Substituted { value } = out else {
            panic!("expected Substituted variant");
        };
        assert!(!value.is_empty());
    }

    #[tokio::test]
    async fn coreferent_entities_collapse_to_same_fake() {
        let op = fake();
        let source = TextData::new("alice");
        let a = coref_entity(
            builtins::PERSON_NAME.label_ref(),
            source.text.as_str(),
            "ENTITY_42",
        );
        let b = coref_entity(
            builtins::PERSON_NAME.label_ref(),
            source.text.as_str(),
            "ENTITY_42",
        );
        let out_a = op.apply(&a, &source).await.unwrap();
        let out_b = op.apply(&b, &source).await.unwrap();
        assert_eq!(out_a, out_b);
        let c = coref_entity(
            builtins::PERSON_NAME.label_ref(),
            source.text.as_str(),
            "ENTITY_99",
        );
        let out_c = op.apply(&c, &source).await.unwrap();
        assert_ne!(
            out_a, out_c,
            "different coref id should yield different fake"
        );
    }

    #[tokio::test]
    async fn distinct_entities_get_distinct_fakes() {
        let op = fake();
        let source = TextData::new("seed");
        let mut outputs: HashSet<String> = HashSet::new();
        for _ in 0..32 {
            let entity = entity_over(builtins::PERSON_NAME.label_ref(), source.text.as_str());
            let out = op.apply(&entity, &source).await.unwrap();
            let TextReplacement::Substituted { value } = out else {
                panic!("expected Substituted");
            };
            outputs.insert(value);
        }
        assert_eq!(
            outputs.len(),
            32,
            "expected 32 distinct fakes across 32 fresh entity ids"
        );
    }

    #[tokio::test]
    async fn tabular_impl_emits_substituted() {
        let op = fake();
        let entity = Entity::<Tabular>::builder()
            .with_label(builtins::PERSON_NAME.label_ref())
            .with_location(nvisy_core::modality::TabularLocation {
                row_index: 0u32,
                column_index: 0u32,
                start_offset: None,
                end_offset: None,
                column_name: None,
                sheet_name: None,
            })
            .with_confidence(nvisy_core::primitive::Confidence::clamped(1.0))
            .build()
            .unwrap();
        let out = op.apply(&entity, &TextData::new("alice")).await.unwrap();
        let TabularReplacement::Substituted { value } = out else {
            panic!("expected Substituted");
        };
        assert!(!value.is_empty());
    }
}
