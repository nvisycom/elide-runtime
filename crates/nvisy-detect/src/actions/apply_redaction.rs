//! Action that applies pending redactions to document text.

use std::any::Any;
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::datatypes::document::Document;
use nvisy_core::datatypes::entity::Entity;
use nvisy_core::datatypes::redaction::Redaction;
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::traits::action::Action;

/// Applies pending [`Redaction`] artifacts to document content.
///
/// The action correlates entities with their redactions, locates the
/// corresponding text spans inside each document, and replaces them with
/// the computed replacement values. The resulting redacted documents are
/// re-emitted as `"documents"` artifacts.
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
    fn id(&self) -> &str {
        "apply-redaction"
    }

    fn validate_params(&self, _params: &serde_json::Value) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        _params: serde_json::Value,
        _client: Option<Box<dyn Any + Send>>,
    ) -> Result<u64, Error> {
        let mut count = 0u64;

        while let Some(mut blob) = input.recv().await {
            let documents: Vec<Document> = blob.get_artifacts("documents").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read documents artifact: {e}"))
            })?;
            let entities: Vec<Entity> = blob.get_artifacts("entities").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read entities artifact: {e}"))
            })?;
            let redactions: Vec<Redaction> = blob.get_artifacts("redactions").map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to read redactions artifact: {e}"))
            })?;

            let entity_map: HashMap<Uuid, &Entity> =
                entities.iter().map(|e| (e.data.id, e)).collect();
            let redaction_map: HashMap<Uuid, &Redaction> =
                redactions.iter().map(|r| (r.entity_id, r)).collect();

            // Clear existing documents -- we will re-add the (possibly redacted) versions
            blob.artifacts.remove("documents");

            for doc in &documents {
                let mut pending: Vec<PendingRedaction> = Vec::new();

                for (entity_id, redaction) in &redaction_map {
                    let entity = match entity_map.get(entity_id) {
                        Some(e) => e,
                        None => continue,
                    };

                    // Check entity belongs to this document
                    let belongs = entity.data.parent_id == Some(doc.data.id)
                        || entity.source_id == Some(doc.data.id);
                    if !belongs {
                        continue;
                    }

                    pending.push(PendingRedaction {
                        start_offset: entity.location.start_offset,
                        end_offset: entity.location.end_offset,
                        replacement_value: redaction.replacement_value.clone(),
                    });
                }

                if pending.is_empty() {
                    blob.add_artifact("documents", doc).map_err(|e| {
                        Error::new(ErrorKind::Runtime, format!("failed to add document artifact: {e}"))
                    })?;
                    count += 1;
                    continue;
                }

                let redacted_content = apply_redactions(&doc.content, &mut pending);
                let mut result = Document::new(redacted_content);
                result.title = doc.title.clone();
                result.elements = doc.elements.clone();
                result.source_format = doc.source_format.clone();
                result.page_count = doc.page_count;
                result.data.parent_id = Some(doc.data.id);

                blob.add_artifact("documents", &result).map_err(|e| {
                    Error::new(ErrorKind::Runtime, format!("failed to add document artifact: {e}"))
                })?;
                count += 1;
            }

            if output.send(blob).await.is_err() {
                return Ok(count);
            }
        }

        Ok(count)
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
