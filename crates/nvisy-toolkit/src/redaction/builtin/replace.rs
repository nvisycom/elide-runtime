//! [`Replace`]: substitute the matched span with a fixed template
//! string.
//!
//! Templates support two placeholders that are expanded at apply
//! time:
//!
//! - `{entity_kind}` — the entity's [`EntityKind`] in snake_case
//!   (e.g. `person_name`).
//! - `{value}` — the original matched substring.
//!
//! The default template is `[{entity_kind}]`.

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Text, TextData};

use super::text_value::read_value;
use crate::redaction::{Anonymizer, LeakProfile, TextReplacement};

/// Substitute the matched span with a template string.
#[derive(Debug, Clone)]
pub struct Replace {
    template: String,
}

impl Replace {
    /// Build a `Replace` operator with the given template. See the
    /// module docs for placeholder syntax.
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
        }
    }
}

impl Default for Replace {
    /// Default template is `[{entity_kind}]` so users who don't
    /// configure a template still get a visible kind-tagged marker.
    fn default() -> Self {
        Self::new("[{entity_kind}]")
    }
}

#[async_trait]
impl Anonymizer<Text> for Replace {
    fn leak_profile(&self) -> LeakProfile {
        // Position and length of the rewritten span are still
        // observable in the output even though the original value is
        // gone.
        LeakProfile::Partial
    }

    async fn apply(&self, entity: &Entity<Text>, source: &TextData) -> Result<TextReplacement> {
        let value = read_value(entity, source);
        let kind = entity.entity_kind.to_string();
        let rendered = render(&self.template, &kind, value);
        Ok(TextReplacement::substituted(rendered))
    }
}

fn render(template: &str, kind: &str, value: &str) -> String {
    template
        .replace("{entity_kind}", kind)
        .replace("{value}", value)
}

#[cfg(test)]
mod tests {
    use nvisy_core::entity::{EntityKind, TrailStep};
    use nvisy_core::modality::TextLocation;
    use nvisy_core::primitive::Confidence;

    use super::*;

    fn entity(kind: EntityKind, start: usize, end: usize) -> Entity<Text> {
        Entity::builder()
            .with_entity_kind(kind)
            .with_location(TextLocation::new(start, end))
            .with_confidence(Confidence::new(1.0).unwrap())
            .with_trail(Vec::<TrailStep>::new())
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn default_template_emits_bracketed_kind() {
        let op = Replace::default();
        let source = TextData::new("alice@example.test");
        let entity = entity(EntityKind::EmailAddress, 0, 18);
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("[email_address]"));
    }

    #[tokio::test]
    async fn template_with_value_placeholder() {
        let op = Replace::new("<<{value}::{entity_kind}>>");
        let source = TextData::new("alice@example.test");
        let entity = entity(EntityKind::EmailAddress, 0, 5);
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(
            out,
            TextReplacement::substituted("<<alice::email_address>>")
        );
    }

    #[tokio::test]
    async fn out_of_bounds_location_yields_empty_value() {
        let op = Replace::new("[{value}]");
        let source = TextData::new("hi");
        let entity = entity(EntityKind::PersonName, 100, 200);
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("[]"));
    }
}
