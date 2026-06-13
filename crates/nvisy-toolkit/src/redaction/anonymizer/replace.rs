//! [`Replace`]: substitute the matched span with a fixed template
//! string.
//!
//! Templates support two placeholders that are expanded at apply
//! time:
//!
//! - `{label}` — the entity's label name (e.g. `person_name`).
//! - `{value}` — the original matched substring.
//!
//! The default template is `[{label}]`.

use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Tabular, Text, TextData};

use crate::redaction::{Anonymizer, LeakProfile, TabularReplacement, TextReplacement};

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
    /// Default template is `[{label}]` so users who don't
    /// configure a template still get a visible label-tagged
    /// marker.
    fn default() -> Self {
        Self::new("[{label}]")
    }
}

#[async_trait::async_trait]
impl Anonymizer<Text> for Replace {
    fn leak_profile(&self) -> LeakProfile {
        // Position and length of the rewritten span are still
        // observable in the output even though the original value is
        // gone.
        LeakProfile::Partial
    }

    async fn apply(&self, entity: &Entity<Text>, source: &TextData) -> Result<TextReplacement> {
        let value = source.text.as_str();
        let rendered = render(&self.template, entity.label.as_str(), value);
        Ok(TextReplacement::substituted(rendered))
    }
}

#[async_trait::async_trait]
impl Anonymizer<Tabular> for Replace {
    fn leak_profile(&self) -> LeakProfile {
        LeakProfile::Partial
    }

    async fn apply(
        &self,
        entity: &Entity<Tabular>,
        source: &TextData,
    ) -> Result<TabularReplacement> {
        let value = source.text.as_str();
        let rendered = render(&self.template, entity.label.as_str(), value);
        Ok(TabularReplacement::substituted(rendered))
    }
}

fn render(template: &str, label: &str, value: &str) -> String {
    template.replace("{label}", label).replace("{value}", value)
}

#[cfg(test)]
mod tests {
    use nvisy_core::entity::{EntityLabelRef, TrailStep, builtins};
    use nvisy_core::modality::TextLocation;
    use nvisy_core::primitive::Confidence;

    use super::*;

    fn entity(label: EntityLabelRef, start: usize, end: usize) -> Entity<Text> {
        Entity::builder()
            .with_label(label)
            .with_location(TextLocation::new(start, end))
            .with_confidence(Confidence::new(1.0).unwrap())
            .with_trail(Vec::<TrailStep>::new())
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn default_template_emits_bracketed_label() {
        let op = Replace::default();
        let source = TextData::new("alice@example.test");
        let entity = entity(builtins::EMAIL_ADDRESS.label_ref(), 0, 18);
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("[email_address]"));
    }

    #[tokio::test]
    async fn template_with_value_placeholder() {
        let op = Replace::new("<<{value}::{label}>>");
        let source = TextData::new("alice");
        let entity = entity(builtins::EMAIL_ADDRESS.label_ref(), 0, source.text.len());
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(
            out,
            TextReplacement::substituted("<<alice::email_address>>")
        );
    }

    #[tokio::test]
    async fn empty_source_yields_empty_value_placeholder() {
        let op = Replace::new("[{value}]");
        let source = TextData::new("");
        let entity = entity(builtins::PERSON_NAME.label_ref(), 0, 0);
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(out, TextReplacement::substituted("[]"));
    }
}
