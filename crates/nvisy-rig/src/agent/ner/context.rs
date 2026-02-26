//! Input context for NER detection calls.

use super::{KnownNerEntity, NerEntity};

/// Input context for a single NER detection call.
///
/// Bundles the text to analyse together with any previously identified
/// entities so the LLM can assign consistent `entity_id` values across
/// chunks or sequential calls.
///
/// Use [`merge`](Self::merge) to accumulate entities from successive
/// detection calls, then update the text with [`set_text`](Self::set_text)
/// before the next call.
pub struct NerContext<'a> {
    /// The text to analyse.
    pub text: &'a str,
    /// Accumulated known entities from prior detection calls.
    pub known_entities: Vec<KnownNerEntity>,
}

impl<'a> NerContext<'a> {
    /// Create a context with no known entities.
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            known_entities: Vec::new(),
        }
    }

    /// Create a context with previously identified entities.
    pub fn with_known(text: &'a str, known_entities: Vec<KnownNerEntity>) -> Self {
        Self {
            text,
            known_entities,
        }
    }

    /// Set the text to analyse, keeping accumulated known entities.
    pub fn set_text(&mut self, text: &'a str) {
        self.text = text;
    }

    /// Merge newly detected entities into the known set.
    ///
    /// For each entity: if a [`KnownNerEntity`] with the same `entity_id`
    /// already exists, its `values` list is extended with any new surface
    /// forms and new descriptions are appended. Otherwise a new
    /// `KnownNerEntity` is created.
    pub fn merge(&mut self, entities: Vec<NerEntity>) {
        for entity in entities {
            if let Some(known) = self
                .known_entities
                .iter_mut()
                .find(|k| k.entity_id == entity.entity_id)
            {
                // Add new surface form if not already present.
                if !known.values.iter().any(|v| v == &entity.value) {
                    known.values.push(entity.value);
                }

                // Append new description if not already present.
                if let Some(desc) = entity.description
                    && !known.descriptions.iter().any(|d| d == &desc)
                {
                    known.descriptions.push(desc);
                }

                // Fill in entity_type if it was previously unknown.
                if known.entity_type.is_none() {
                    known.entity_type = entity.entity_type;
                }
            } else {
                self.known_entities.push(KnownNerEntity {
                    entity_id: entity.entity_id,
                    entity_type: entity.entity_type,
                    values: vec![entity.value],
                    descriptions: entity.description.into_iter().collect(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nvisy_ontology::entity::EntityKind;

    fn ner_entity(id: &str, value: &str, desc: Option<&str>) -> NerEntity {
        NerEntity {
            entity_id: id.into(),
            category: None,
            entity_type: Some(EntityKind::PersonName),
            value: value.into(),
            confidence: None,
            context: None,
            description: desc.map(Into::into),
        }
    }

    #[test]
    fn merge_creates_new_known_entity() {
        let mut ctx = NerContext::new("");
        ctx.merge(vec![ner_entity("person_1", "John Smith", Some("the CEO"))]);

        assert_eq!(ctx.known_entities.len(), 1);
        assert_eq!(ctx.known_entities[0].entity_id, "person_1");
        assert_eq!(ctx.known_entities[0].values, vec!["John Smith"]);
        assert_eq!(ctx.known_entities[0].descriptions, vec!["the CEO"]);
    }

    #[test]
    fn merge_accumulates_surface_forms() {
        let mut ctx = NerContext::new("");
        ctx.merge(vec![ner_entity("person_1", "John Smith", None)]);
        ctx.merge(vec![ner_entity("person_1", "John", None)]);
        ctx.merge(vec![ner_entity("person_1", "Mr. Smith", None)]);
        // Duplicate value should not be added.
        ctx.merge(vec![ner_entity("person_1", "John", None)]);

        assert_eq!(ctx.known_entities.len(), 1);
        assert_eq!(
            ctx.known_entities[0].values,
            vec!["John Smith", "John", "Mr. Smith"],
        );
    }

    #[test]
    fn merge_accumulates_descriptions() {
        let mut ctx = NerContext::new("");
        ctx.merge(vec![ner_entity("person_1", "Alice", Some("the CEO"))]);
        ctx.merge(vec![ner_entity("person_1", "Alice", Some("signed the contract on Jan 5"))]);

        assert_eq!(
            ctx.known_entities[0].descriptions,
            vec!["the CEO", "signed the contract on Jan 5"],
        );
    }

    #[test]
    fn merge_deduplicates_descriptions() {
        let mut ctx = NerContext::new("");
        ctx.merge(vec![ner_entity("person_1", "Alice", Some("the CEO"))]);
        ctx.merge(vec![ner_entity("person_1", "Alice", Some("the CEO"))]);

        assert_eq!(ctx.known_entities[0].descriptions, vec!["the CEO"]);
    }

    #[test]
    fn merge_no_description() {
        let mut ctx = NerContext::new("");
        ctx.merge(vec![ner_entity("person_1", "Alice", None)]);

        assert!(ctx.known_entities[0].descriptions.is_empty());
    }

    #[test]
    fn merge_fills_missing_entity_type() {
        let mut ctx = NerContext::new("");
        let mut e = ner_entity("org_1", "Acme", None);
        e.entity_type = None;
        ctx.merge(vec![e]);
        assert!(ctx.known_entities[0].entity_type.is_none());

        ctx.merge(vec![ner_entity("org_1", "Acme Corp", None)]);
        assert_eq!(ctx.known_entities[0].entity_type, Some(EntityKind::PersonName));
    }
}
