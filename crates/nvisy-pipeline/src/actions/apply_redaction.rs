//! Action that applies pending redactions to document text.

use std::collections::HashMap;
use uuid::Uuid;

use nvisy_ingest::handler::{FormatHandler, PlaintextHandler};
use nvisy_ingest::document::Document;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::redaction::Redaction;
use nvisy_core::error::Error;

use crate::action::Action;

/// Applies pending [`Redaction`] instructions to document content.
///
/// The action correlates entities with their redactions, locates the
/// corresponding text spans inside each document, and replaces them with
/// the computed replacement values. The resulting redacted documents are
/// returned.
pub struct ApplyRedactionAction;

/// A single text replacement that has been resolved but not yet applied.
struct PendingRedaction {
    /// Byte offset where the redaction starts in the original text.
    start_offset: usize,
    /// Byte offset where the redaction ends (exclusive) in the original text.
    end_offset: usize,
    /// The string that will replace the original span.
    replacement_value: String,
}

#[async_trait::async_trait]
impl Action for ApplyRedactionAction {
    type Params = ();
    type Input = (Vec<Document<FormatHandler>>, Vec<Entity>, Vec<Redaction>);
    type Output = Vec<Document<FormatHandler>>;

    fn id(&self) -> &str {
        "apply-redaction"
    }

    async fn connect(_params: Self::Params) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Vec<Document<FormatHandler>>, Error> {
        let (documents, entities, redactions) = input;

        let entity_map: HashMap<Uuid, &Entity> =
            entities.iter().map(|e| (e.source.as_uuid(), e)).collect();
        let redaction_map: HashMap<Uuid, &Redaction> =
            redactions.iter().map(|r| (r.entity_id, r)).collect();

        let mut result_docs = Vec::new();

        for doc in &documents {
            let content = match &doc.content {
                Some(c) => c,
                None => {
                    result_docs.push(doc.clone());
                    continue;
                }
            };

            let mut pending: Vec<PendingRedaction> = Vec::new();

            for (entity_id, redaction) in &redaction_map {
                let entity = match entity_map.get(entity_id) {
                    Some(e) => e,
                    None => continue,
                };

                // Check entity belongs to this document
                let belongs = entity.source.parent_id() == Some(doc.source.as_uuid());
                if !belongs {
                    continue;
                }

                let start_offset = match entity.location.start_offset() {
                    Some(s) => s,
                    None => continue,
                };
                let end_offset = match entity.location.end_offset() {
                    Some(e) => e,
                    None => continue,
                };

                let replacement_value = redaction
                    .output
                    .replacement_value()
                    .unwrap_or("")
                    .to_string();

                pending.push(PendingRedaction {
                    start_offset,
                    end_offset,
                    replacement_value,
                });
            }

            if pending.is_empty() {
                result_docs.push(doc.clone());
                continue;
            }

            let redacted_content = apply_redactions(content, &mut pending);
            let mut result = Document::new(FormatHandler::Plaintext(PlaintextHandler))
                .with_text(redacted_content);
            result.title = doc.title.clone();
            result.elements = doc.elements.clone();
            result.page_count = doc.page_count;
            result.source.set_parent_id(Some(doc.source.as_uuid()));

            result_docs.push(result);
        }

        Ok(result_docs)
    }
}

/// Applies a set of pending redactions to `text`, returning the redacted result.
///
/// Replacements are applied right-to-left (descending start offset) so that
/// earlier byte offsets remain valid after each substitution.
fn apply_redactions(text: &str, pending: &mut [PendingRedaction]) -> String {
    // Sort by start offset descending (right-to-left) to preserve positions
    pending.sort_by(|a, b| b.start_offset.cmp(&a.start_offset));

    let mut result = text.to_string();
    for redaction in pending.iter() {
        let start = redaction.start_offset.min(result.len());
        let end = redaction.end_offset.min(result.len());
        if start >= end {
            continue;
        }

        result = format!(
            "{}{}{}",
            &result[..start],
            redaction.replacement_value,
            &result[end..]
        );
    }
    result
}
