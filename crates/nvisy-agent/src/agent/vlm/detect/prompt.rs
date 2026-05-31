//! Prompt construction for VLM image entity detection.

use crate::agent::{ALL_TYPES_HINT, VlmDetectContext};

/// Builds user prompts for the VLM detect pass.
pub(crate) struct VlmDetectPromptBuilder<'a> {
    config: &'a VlmDetectContext,
}

impl<'a> VlmDetectPromptBuilder<'a> {
    pub fn new(config: &'a VlmDetectContext) -> Self {
        Self { config }
    }

    pub fn build(&self, image_b64: &str) -> String {
        let types = if self.config.entity_kinds.is_empty() {
            ALL_TYPES_HINT.to_string()
        } else {
            self.config
                .entity_kinds
                .iter()
                .map(|k| k.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };

        let mut prompt = format!(
            "Find every sensitive entity of types [{types}] visible in the \
             image below. Draw a tight bounding box around each. Return a JSON object \
             with an \"entities\" key whose value is an array of detections."
        );

        if !self.config.labels.is_empty() {
            let labels = self.config.labels.join(", ");
            prompt.push_str(&format!(
                "\n\nDocument context labels (adjust sensitivity to domain-specific \
                 visual content accordingly): {labels}."
            ));
        }

        prompt.push_str("\n\nImage (base64):\n");
        prompt.push_str(image_b64);

        prompt
    }
}

/// Default system prompt for the VLM detect pass.
pub(super) const VLM_DETECT_SYSTEM_PROMPT: &str = "\
You are a precise vision-language entity detector. Given an image, identify \
visible personally identifiable information (PII), protected health information (PHI), \
financial data, credentials, and any other sensitive content.\n\
\n\
For each entity, return one entry in the response with:\n\
- category (broad classification)\n\
- entity_kind (specific type)\n\
- x, y, width, height: bounding box in *normalized* coordinates where \
  (0, 0) is the top-left of the image and (1, 1) is the bottom-right. Box \
  must be tight around the visible content; do not pad.\n\
- confidence: your confidence in [0.0, 1.0] that the box is correctly placed \
  and the entity is what you say it is.\n\
- description (optional): a short human-readable note about what the box \
  contains, e.g. \"woman's face\", \"credit-card number on receipt\".\n\
\n\
Return a JSON object with an \"entities\" key containing the array. If no \
sensitive entities are visible, return {\"entities\": []}.\n\
\n\
Be precise with bounding boxes — they will be used to redact the image. \
A loose box leaks surrounding content; a tight one preserves the most \
non-sensitive context.";
