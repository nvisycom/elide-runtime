//! Verification-specific prompt construction.
//!
//! [`VlmVerifyAgentPromptBuilder`] constructs the user prompt that lists proposed
//! entities for the VLM to verify against the original image.

use super::input::ProposedEntity;

/// Builds user prompts for entity verification.
pub(crate) struct VlmVerifyAgentPromptBuilder<'a> {
    entities: &'a [ProposedEntity],
}

impl<'a> VlmVerifyAgentPromptBuilder<'a> {
    /// Create a prompt builder from a slice of proposed entities.
    pub fn new(entities: &'a [ProposedEntity]) -> Self {
        Self { entities }
    }

    /// Build the user prompt for the given base64-encoded image.
    pub fn build(&self, image_b64: &str) -> String {
        let mut prompt = String::from(
            "Verify the following proposed entities against the image. \
             For each entity that needs correction or should be rejected, \
             include it in your response. Omit entities that are correct.\n\n\
             Proposed entities:\n",
        );

        for entity in self.entities {
            prompt.push_str(&format!(
                "[{}] type={}, value=\"{}\", confidence={:.2}",
                entity.id, entity.entity_type, entity.value, entity.confidence,
            ));
            if let Some(ref bbox) = entity.bbox {
                prompt.push_str(&format!(
                    ", bbox=[{:.1}, {:.1}, {:.1}, {:.1}]",
                    bbox.x, bbox.y, bbox.width, bbox.height,
                ));
            }
            prompt.push('\n');
        }

        prompt.push_str(&format!("\nImage (base64): {image_b64}"));
        prompt
    }
}

/// Default system prompt for the OCR verification agent.
pub(super) const VLM_VERIFIER_SYSTEM_PROMPT: &str = "\
You are a vision-language model that verifies proposed entity detections against an image. \
You receive a list of entities (each with an id, type, value, confidence, and optional \
bounding box) that were detected by an NER system from OCR-extracted text.\n\
\n\
Your task is to look at the image and verify each entity. Return a JSON object with an \
\"entities\" key containing an array of only the entities that need changes:\n\
\n\
- **corrected**: The entity exists but has wrong value or type. Include the corrected \
  fields (entity_type, value, bbox) along with your confidence and an optional reason.\n\
- **rejected**: The entity is a false positive — it does not appear in the image or was \
  misidentified. Include your confidence and an optional reason.\n\
\n\
Entities that are correct should NOT appear in your response. If all entities are correct, \
return {\"entities\": []}.\n\
\n\
Each entry in the array must have: id (matching the proposed entity's id), status \
(\"corrected\" or \"rejected\"), confidence (0.0-1.0). For corrected entities, also include \
whichever fields changed: entity_type, value, bbox.";

#[cfg(test)]
mod tests {
    use nvisy_core::entity::EntityKind;
    use nvisy_core::primitive::BoundingBox;

    use super::*;

    #[test]
    fn builds_prompt_with_entities() {
        let entities = vec![
            ProposedEntity {
                id: 0,
                entity_type: EntityKind::PersonName,
                value: "John Doe".into(),
                confidence: 0.95,
                bbox: Some(BoundingBox {
                    x: 10.0,
                    y: 20.0,
                    width: 100.0,
                    height: 30.0,
                }),
            },
            ProposedEntity {
                id: 1,
                entity_type: EntityKind::PaymentCard,
                value: "4111-1111-1111-1111".into(),
                confidence: 0.80,
                bbox: None,
            },
        ];

        let prompt = VlmVerifyAgentPromptBuilder::new(&entities).build("AAAA");
        assert!(prompt.contains("[0] type=person_name"));
        assert!(prompt.contains("John Doe"));
        assert!(prompt.contains("bbox=[10.0, 20.0, 100.0, 30.0]"));
        assert!(prompt.contains("[1] type=payment_card"));
        assert!(prompt.contains("Image (base64): AAAA"));
    }

    #[test]
    fn builds_prompt_with_no_entities() {
        let prompt = VlmVerifyAgentPromptBuilder::new(&[]).build("BBBB");
        assert!(prompt.contains("Proposed entities:\n"));
        assert!(prompt.contains("Image (base64): BBBB"));
    }
}
