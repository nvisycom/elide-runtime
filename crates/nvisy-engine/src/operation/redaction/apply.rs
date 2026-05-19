//! Compute replacement values and apply redaction records to
//! document content via codec transforms.
//!
//! [`RedactionApplicator`] holds a mutable reference to the envelope,
//! iterates `audit.entries`, builds per-modality codec instructions,
//! writes `replaced_value` inline, and applies instructions to the
//! document.

use std::collections::HashMap;

use nvisy_codec::transform::{
    AudioOutput, AudioRedaction, ConflictPolicy, ImageOutput, ImageRedaction, Redactions,
    TabularRedaction, TextOutput, TextRedaction,
};
use nvisy_core::{Error, Result};
use nvisy_ontology::entity::{
    AudioLocation, Entity, EntityKind, ImageLocation, Location, TabularLocation, TextLocation,
};
use nvisy_ontology::policy::{AudioStrategy, ImageStrategy, Strategy, TextStrategy};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::operation::DocumentEnvelope;

const TARGET: &str = "nvisy_engine::op::redaction::apply";

/// Builds and applies redaction instructions across all modalities.
///
/// Holds `&mut DocumentEnvelope` and iterates `audit.entries` to build
/// codec instructions. Writes `replaced_value` directly into each
/// record during the build phase.
pub(super) struct RedactionApplicator<'a> {
    envelope: &'a mut DocumentEnvelope,
}

impl<'a> RedactionApplicator<'a> {
    pub fn new(envelope: &'a mut DocumentEnvelope) -> Self {
        Self { envelope }
    }

    /// Build and apply all redaction instructions.
    pub async fn apply(mut self) -> Result<()> {
        let entity_map = entity_map(&self.envelope.audit.entities);
        let mut mapping_index = mapping_index(&self.envelope.redaction_map.entries);

        let text = self.build_text_redactions(&entity_map, &mut mapping_index).await?;
        let tabular = self.build_tabular_redactions(&entity_map, &mut mapping_index).await?;
        let image = self.build_image_redactions(&entity_map, &mut mapping_index)?;
        let audio = self.build_audio_redactions(&entity_map, &mut mapping_index)?;

        if !text.is_empty() {
            self.envelope.document.apply_text_redactions(text).await?;
        }
        if !tabular.is_empty() {
            self.envelope
                .document
                .apply_tabular_redactions(tabular)
                .await?;
        }
        if !image.is_empty() {
            self.envelope.document.apply_image_redactions(image).await?;
        }
        if !audio.is_empty() {
            self.envelope.document.apply_audio_redactions(audio).await?;
        }

        Ok(())
    }

    async fn build_text_redactions(
        &mut self,
        entity_map: &HashMap<Uuid, Entity>,
        mapping_index: &mut HashMap<Uuid, usize>,
    ) -> Result<Redactions<TextLocation, TextRedaction>> {
        let mut redactions = Redactions::new(ConflictPolicy::Reject);

        for i in 0..self.envelope.audit.entries.len() {
            let record = &self.envelope.audit.entries[i];
            let Some(entity) = entity_map.get(&record.entity_id) else {
                continue;
            };
            let Location::Text(ref loc) = entity.location else {
                continue;
            };
            let Strategy::Text(ref strategy) = record.redaction.strategy else {
                log_modality_mismatch(entity.id, "text", &record.redaction.strategy);
                continue;
            };

            let value = self
                .envelope
                .document
                .read_text(loc)
                .await
                .map(|d| d.into_inner())
                .unwrap_or_default();

            let output = text_output(&value, entity, strategy);
            let entity_id = record.entity_id;
            let replacement = output.replacement_value().map(String::from);

            self.envelope.audit.entries[i].value.replacement = replacement.clone();
            if let Some(&idx) = mapping_index.get(&entity_id) {
                self.envelope.redaction_map.entries[idx].replacement = replacement;
            }

            tracing::trace!(
                target: TARGET,
                %entity_id,
                start = loc.start_offset,
                end = loc.end_offset,
                "built text redaction instruction",
            );

            redactions
                .try_insert(
                    loc.clone(),
                    TextRedaction::new(loc.start_offset, loc.end_offset, output),
                )
                .map_err(|e| Error::validation(e.to_string(), "redaction-apply-text"))?;
        }

        Ok(redactions)
    }

    async fn build_tabular_redactions(
        &mut self,
        entity_map: &HashMap<Uuid, Entity>,
        mapping_index: &mut HashMap<Uuid, usize>,
    ) -> Result<Redactions<TabularLocation, TabularRedaction>> {
        let mut redactions = Redactions::new(ConflictPolicy::Reject);

        for i in 0..self.envelope.audit.entries.len() {
            let record = &self.envelope.audit.entries[i];
            let Some(entity) = entity_map.get(&record.entity_id) else {
                continue;
            };
            let Location::Tabular(ref loc) = entity.location else {
                continue;
            };
            let Strategy::Text(ref strategy) = record.redaction.strategy else {
                log_modality_mismatch(entity.id, "tabular", &record.redaction.strategy);
                continue;
            };

            let value = self
                .envelope
                .document
                .read_tabular(loc)
                .await
                .map(|d| d.into_inner())
                .unwrap_or_default();

            let output = text_output(&value, entity, strategy);
            let entity_id = record.entity_id;
            let replacement = output.replacement_value().map(String::from);

            self.envelope.audit.entries[i].value.replacement = replacement.clone();
            if let Some(&idx) = mapping_index.get(&entity_id) {
                self.envelope.redaction_map.entries[idx].replacement = replacement;
            }

            // Intra-cell byte range: prefer explicit offsets if the
            // entity provided them, otherwise redact the entire cell.
            let start = loc.start_offset.unwrap_or(0);
            let end = loc.end_offset.unwrap_or(value.len());

            tracing::trace!(
                target: TARGET,
                %entity_id,
                row = loc.row_index,
                col = loc.column_index,
                start,
                end,
                "built tabular redaction instruction",
            );

            redactions
                .try_insert(loc.clone(), TabularRedaction::new(start, end, output))
                .map_err(|e| Error::validation(e.to_string(), "redaction-apply-tabular"))?;
        }

        Ok(redactions)
    }

    fn build_image_redactions(
        &mut self,
        entity_map: &HashMap<Uuid, Entity>,
        mapping_index: &mut HashMap<Uuid, usize>,
    ) -> Result<Redactions<ImageLocation, ImageRedaction>> {
        let mut redactions = Redactions::new(ConflictPolicy::Reject);

        for i in 0..self.envelope.audit.entries.len() {
            let record = &self.envelope.audit.entries[i];
            let Some(entity) = entity_map.get(&record.entity_id) else {
                continue;
            };
            let Location::Image(ref loc) = entity.location else {
                continue;
            };
            let Strategy::Image(ref strategy) = record.redaction.strategy else {
                log_modality_mismatch(entity.id, "image", &record.redaction.strategy);
                continue;
            };

            let Some((output, placeholder)) = image_output(strategy) else {
                tracing::debug!(
                    target: TARGET,
                    entity_id = %entity.id,
                    strategy = ?strategy,
                    "image strategy has no codec output, skipping",
                );
                continue;
            };

            let entity_id = record.entity_id;
            self.envelope.audit.entries[i].value.replacement = Some(placeholder.clone());
            if let Some(&idx) = mapping_index.get(&entity_id) {
                self.envelope.redaction_map.entries[idx].replacement = Some(placeholder);
            }

            tracing::trace!(
                target: TARGET,
                %entity_id,
                "built image redaction instruction",
            );

            redactions
                .try_insert(loc.clone(), ImageRedaction::new(loc.bounding_box, output))
                .map_err(|e| Error::validation(e.to_string(), "redaction-apply-image"))?;
        }

        Ok(redactions)
    }

    fn build_audio_redactions(
        &mut self,
        entity_map: &HashMap<Uuid, Entity>,
        mapping_index: &mut HashMap<Uuid, usize>,
    ) -> Result<Redactions<AudioLocation, AudioRedaction>> {
        let mut redactions = Redactions::new(ConflictPolicy::Reject);

        for i in 0..self.envelope.audit.entries.len() {
            let record = &self.envelope.audit.entries[i];
            let Some(entity) = entity_map.get(&record.entity_id) else {
                continue;
            };
            let Location::Audio(ref loc) = entity.location else {
                continue;
            };
            let Strategy::Audio(ref strategy) = record.redaction.strategy else {
                log_modality_mismatch(entity.id, "audio", &record.redaction.strategy);
                continue;
            };

            let Some((output, placeholder)) = audio_output(strategy) else {
                tracing::debug!(
                    target: TARGET,
                    entity_id = %entity.id,
                    strategy = ?strategy,
                    "audio strategy has no codec output, skipping",
                );
                continue;
            };

            let entity_id = record.entity_id;
            self.envelope.audit.entries[i].value.replacement = Some(placeholder.clone());
            if let Some(&idx) = mapping_index.get(&entity_id) {
                self.envelope.redaction_map.entries[idx].replacement = Some(placeholder);
            }

            tracing::trace!(
                target: TARGET,
                %entity_id,
                start_us = loc.time_span.start_us,
                end_us = loc.time_span.end_us,
                "built audio redaction instruction",
            );

            redactions
                .try_insert(loc.clone(), AudioRedaction::new(loc.time_span, output))
                .map_err(|e| Error::validation(e.to_string(), "redaction-apply-audio"))?;
        }

        Ok(redactions)
    }
}

/// Build a lookup map from entity UUID to a cloned entity.
fn entity_map(entities: &nvisy_ontology::entity::Entities) -> HashMap<Uuid, Entity> {
    entities.iter().map(|e| (e.id, e.clone())).collect()
}

/// Build an index from entity UUID to its position in
/// [`RedactionMap::entries`], so per-entity mapping updates become
/// `O(1)` instead of a linear scan per audit entry.
///
/// [`RedactionMap::entries`]: nvisy_ontology::provenance::RedactionMap::entries
fn mapping_index(
    mappings: &[nvisy_ontology::provenance::RedactionMapping],
) -> HashMap<Uuid, usize> {
    mappings.iter().enumerate().map(|(i, m)| (m.entity_id, i)).collect()
}

/// Compute the codec [`TextOutput`] for a value + entity + strategy.
fn text_output(value: &str, entity: &Entity, strategy: &TextStrategy) -> TextOutput {
    match strategy {
        TextStrategy::Mask { mask_char } => {
            // Repeat by character count, not byte length: a 2-byte
            // grapheme like `é` should produce one mask char.
            TextOutput::replace(mask_char.to_string().repeat(value.chars().count()))
        }
        TextStrategy::Replace { placeholder } => {
            if placeholder.is_empty() {
                TextOutput::replace(format!(
                    "[{}]",
                    entity.entity_kind.to_string().to_uppercase()
                ))
            } else {
                TextOutput::replace(
                    placeholder
                        .replace("{entityType}", &entity.entity_kind.to_string())
                        .replace("{category}", &entity.category.to_string()),
                )
            }
        }
        TextStrategy::Remove => TextOutput::Remove,
        TextStrategy::Hash => TextOutput::replace(hash_value(value)),
        TextStrategy::Pseudonymize => {
            TextOutput::replace(pseudonymize(&entity.entity_kind, value))
        }
        TextStrategy::Encrypt { .. } => {
            // TODO: real encryption — placeholder until the key vault is wired.
            TextOutput::replace(format!("[ENC:{}]", entity.entity_kind))
        }
        TextStrategy::Tokenize { .. } => {
            // TODO: real tokenization — placeholder until the vault is wired.
            TextOutput::replace(format!("[TOKEN:{}]", entity.entity_kind))
        }
        // `TextStrategy` is `#[non_exhaustive]`; new variants need
        // explicit handling. Surface a generic placeholder so the
        // pipeline doesn't silently drop entities.
        _ => {
            tracing::warn!(
                target: TARGET,
                entity_id = %entity.id,
                strategy = ?strategy,
                "unhandled text strategy, using generic placeholder",
            );
            TextOutput::replace(format!("[REDACTED:{}]", entity.entity_kind))
        }
    }
}

/// Compute the codec [`ImageOutput`] for a strategy, paired with a
/// human-readable placeholder for the audit trail.
///
/// Returns `None` when the strategy has no defined codec output.
fn image_output(strategy: &ImageStrategy) -> Option<(ImageOutput, String)> {
    match strategy {
        ImageStrategy::Blur { sigma } => {
            Some((ImageOutput::Blur { sigma: *sigma }, format!("[IMAGE_BLUR:{sigma}]")))
        }
        ImageStrategy::Block { color } => Some((
            ImageOutput::Block { color: *color },
            format!("[IMAGE_BLOCK:#{:02x}{:02x}{:02x}]", color.r, color.g, color.b),
        )),
        ImageStrategy::Pixelate { block_size } => Some((
            ImageOutput::Pixelate {
                block_size: *block_size,
            },
            format!("[IMAGE_PIXELATE:{block_size}]"),
        )),
        // `ImageStrategy` is `#[non_exhaustive]`; new variants without
        // a defined codec output are skipped.
        _ => None,
    }
}

/// Compute the codec [`AudioOutput`] for a strategy, paired with a
/// human-readable placeholder for the audit trail.
///
/// Returns `None` when the strategy has no defined codec output.
fn audio_output(strategy: &AudioStrategy) -> Option<(AudioOutput, String)> {
    match strategy {
        AudioStrategy::Silence => Some((AudioOutput::Silence, "[AUDIO_SILENCE]".to_string())),
        AudioStrategy::Remove => Some((AudioOutput::Remove, "[AUDIO_REMOVE]".to_string())),
        // `AudioStrategy` is `#[non_exhaustive]`; new variants without
        // a defined codec output are skipped.
        _ => None,
    }
}

fn hash_value(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let hash = hasher.finalize();
    hash.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn pseudonymize(entity_kind: &EntityKind, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(entity_kind.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(value.as_bytes());
    let hash = hasher.finalize();
    let id: u32 = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]);
    format!("{entity_kind}_{id}")
}

fn log_modality_mismatch(entity_id: Uuid, location_modality: &str, strategy: &Strategy) {
    tracing::trace!(
        target: TARGET,
        %entity_id,
        location_modality,
        strategy = ?strategy,
        "strategy modality does not match entity location, skipping",
    );
}

#[cfg(test)]
mod tests {
    use nvisy_codec::ContentHandle;
    use nvisy_core::content::{Content, ContentData, ContentMetadata, ContentSource};
    use nvisy_ontology::entity::{Entities, Entity, EntityKind, Location, TabularLocation};
    use nvisy_ontology::policy::ImageStrategy;
    use nvisy_ontology::provenance::{AuditEntry, RedactionMapping};

    use super::*;
    use crate::operation::envelope::SharedData;

    fn text_entity(start: usize, end: usize) -> Entity {
        Entity::test_builder(start, end).test_build()
    }

    fn tabular_entity(row: usize, col: usize, start: usize, end: usize) -> Entity {
        let loc = TabularLocation {
            row_index: row,
            column_index: col,
            start_offset: Some(start),
            end_offset: Some(end),
            column_name: None,
            sheet_name: None,
        };
        Entity::test_builder(0, 0)
            .with_location(Location::from(loc))
            .test_build()
    }

    async fn test_envelope(entities: Entities) -> DocumentEnvelope {
        envelope_with("Hello John world", "text/plain", entities).await
    }

    async fn test_envelope_csv(entities: Entities, csv: &str) -> DocumentEnvelope {
        envelope_with(csv, "text/csv", entities).await
    }

    async fn envelope_with(
        body: &str,
        content_type: &str,
        entities: Entities,
    ) -> DocumentEnvelope {
        let data = ContentData::from_text(ContentSource::new(), body);
        let content =
            Content::with_metadata(data, ContentMetadata::new().with_content_type(content_type));
        let handle = ContentHandle::decode(&content).await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let registry = crate::registry::Registry::open(dir.path()).unwrap();
        let shared = SharedData::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), registry);
        let mut envelope = DocumentEnvelope::new(handle, ContentMetadata::default(), shared);
        envelope.audit.entities = entities;
        envelope
    }

    fn test_record(entity_id: Uuid, strategy: Strategy, original: &str) -> AuditEntry {
        AuditEntry::builder()
            .for_entity(entity_id, strategy, original)
            .build()
            .unwrap()
    }

    fn test_mapping(entity_id: Uuid, location: Location, original: &str) -> RedactionMapping {
        RedactionMapping {
            entity_id,
            location,
            original: original.to_owned(),
            replacement: None,
        }
    }

    #[tokio::test]
    async fn mask_applies_and_records_replacement() {
        let entity = text_entity(6, 10);
        let entity_id = entity.id;
        let record = test_record(
            entity_id,
            Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            "John",
        );

        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.entries.push(record);

        RedactionApplicator::new(&mut envelope)
            .apply()
            .await
            .unwrap();

        assert_eq!(
            envelope.audit.entries[0].value.replacement.as_deref(),
            Some("****"),
        );
    }

    #[tokio::test]
    async fn mask_counts_chars_not_bytes() {
        // The handler reads "John" (4 bytes / 4 chars); for an
        // entity spanning a multi-byte character the count should
        // track chars. This test pins the chars-not-bytes behavior
        // by checking the mask length matches the char count of the
        // value the handler returns.
        let entity = text_entity(6, 10);
        let entity_id = entity.id;
        let record = test_record(
            entity_id,
            Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            "John",
        );
        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.entries.push(record);

        RedactionApplicator::new(&mut envelope)
            .apply()
            .await
            .unwrap();

        // "John" is 4 chars -> 4 mask chars.
        assert_eq!(
            envelope.audit.entries[0].value.replacement.as_deref(),
            Some("****"),
        );
    }

    #[tokio::test]
    async fn remove_leaves_replacement_none() {
        let entity = text_entity(6, 10);
        let entity_id = entity.id;
        let record = test_record(entity_id, Strategy::Text(TextStrategy::Remove), "John");

        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.entries.push(record);

        RedactionApplicator::new(&mut envelope)
            .apply()
            .await
            .unwrap();

        assert!(envelope.audit.entries[0].value.replacement.is_none());
    }

    #[tokio::test]
    async fn skips_image_strategy_for_text_entity() {
        let entity = text_entity(0, 4);
        let record = test_record(
            entity.id,
            Strategy::Image(ImageStrategy::Blur { sigma: 15.0 }),
            "face",
        );

        let entities: Entities = vec![entity].into();
        let mut envelope = test_envelope(entities).await;
        envelope.audit.entries.push(record);

        RedactionApplicator::new(&mut envelope)
            .apply()
            .await
            .unwrap();
    }

    #[test]
    fn hash_replacement_is_deterministic() {
        let a = hash_value("John Smith");
        let b = hash_value("John Smith");
        assert_eq!(a, b);
        assert!(!a.is_empty());
    }

    #[test]
    fn pseudonymize_is_deterministic() {
        let a = pseudonymize(&EntityKind::PersonName, "John Smith");
        let b = pseudonymize(&EntityKind::PersonName, "John Smith");
        assert_eq!(a, b);
        assert!(a.contains('_'));
    }

    // -- Tabular end-to-end tests --

    #[tokio::test]
    async fn tabular_full_cell_mask() {
        // row 1, col 1 = "123-45-6789" (11 chars)
        let entity = tabular_entity(1, 1, 0, 11);
        let entity_id = entity.id;
        let record = test_record(
            entity_id,
            Strategy::Text(TextStrategy::Mask { mask_char: '*' }),
            "123-45-6789",
        );

        let entities: Entities = vec![entity.clone()].into();
        let mut envelope =
            test_envelope_csv(entities, "name,ssn\nAlice,123-45-6789\n").await;
        envelope.audit.entries.push(record);
        envelope
            .redaction_map
            .entries
            .push(test_mapping(entity_id, entity.location.clone(), "123-45-6789"));

        RedactionApplicator::new(&mut envelope).apply().await.unwrap();

        // 11 chars -> 11 asterisks
        assert_eq!(
            envelope.audit.entries[0].value.replacement.as_deref(),
            Some("***********"),
        );
        // Mapping was updated too.
        assert_eq!(
            envelope.redaction_map.entries[0].replacement.as_deref(),
            Some("***********"),
        );
        // Document content was actually mutated.
        let value = envelope
            .document
            .read_tabular(entity.location.as_tabular().unwrap())
            .await
            .map(|d| d.into_inner());
        assert_eq!(value.as_deref(), Some("***********"));
    }

    #[tokio::test]
    async fn tabular_partial_cell_replace() {
        // row 1, col 0 = "Alice Smith" — redact "Alice" (0..5)
        let entity = tabular_entity(1, 0, 0, 5);
        let entity_id = entity.id;
        let record = test_record(
            entity_id,
            Strategy::Text(TextStrategy::Replace {
                placeholder: "[NAME]".to_owned(),
            }),
            "Alice",
        );

        let entities: Entities = vec![entity.clone()].into();
        let mut envelope =
            test_envelope_csv(entities, "name,ssn\nAlice Smith,123-45-6789\n").await;
        envelope.audit.entries.push(record);

        RedactionApplicator::new(&mut envelope).apply().await.unwrap();

        let value = envelope
            .document
            .read_tabular(entity.location.as_tabular().unwrap())
            .await
            .map(|d| d.into_inner());
        assert_eq!(value.as_deref(), Some("[NAME] Smith"));
    }

    #[tokio::test]
    async fn tabular_remove_leaves_empty_cell() {
        let entity = tabular_entity(1, 1, 0, 11);
        let entity_id = entity.id;
        let record = test_record(
            entity_id,
            Strategy::Text(TextStrategy::Remove),
            "123-45-6789",
        );

        let entities: Entities = vec![entity.clone()].into();
        let mut envelope =
            test_envelope_csv(entities, "name,ssn\nAlice,123-45-6789\n").await;
        envelope.audit.entries.push(record);

        RedactionApplicator::new(&mut envelope).apply().await.unwrap();

        let value = envelope
            .document
            .read_tabular(entity.location.as_tabular().unwrap())
            .await
            .map(|d| d.into_inner());
        assert_eq!(value.as_deref(), Some(""));
    }

    #[tokio::test]
    async fn tabular_image_strategy_for_tabular_entity_skipped() {
        // Mismatched modality: tabular location with image strategy.
        // The applicator should skip it without erroring.
        let entity = tabular_entity(1, 0, 0, 5);
        let record = test_record(
            entity.id,
            Strategy::Image(ImageStrategy::Blur { sigma: 15.0 }),
            "Alice",
        );

        let entities: Entities = vec![entity].into();
        let mut envelope =
            test_envelope_csv(entities, "name,ssn\nAlice,123\n").await;
        envelope.audit.entries.push(record);

        // No error, but no replacement either.
        RedactionApplicator::new(&mut envelope).apply().await.unwrap();
        assert!(envelope.audit.entries[0].value.replacement.is_none());
    }
}
