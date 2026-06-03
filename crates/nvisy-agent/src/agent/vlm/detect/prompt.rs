//! Prompt construction for VLM image entity detection.
//!
//! Built from the per-call image + uploader-supplied hints + document
//! labels. Hints carry image-space bounding boxes the uploader marked
//! as likely sensitive, so the model can confirm or relocate them
//! alongside open-ended discovery.

use nvisy_core::Hint;
use nvisy_core::modality::Image;

/// Builds user prompts for the VLM detect pass from a per-call
/// image payload, the uploader-supplied hints, and the document
/// labels.
pub(crate) struct VlmDetectPromptBuilder<'a> {
    hints: &'a [Hint<Image>],
    labels: &'a [String],
}

impl<'a> VlmDetectPromptBuilder<'a> {
    pub fn new(hints: &'a [Hint<Image>], labels: &'a [String]) -> Self {
        Self { hints, labels }
    }

    pub fn build(&self, image_b64: &str) -> String {
        let mut prompt = String::from(
            "Find every sensitive entity visible in the image below. \
             Draw a tight bounding box around each. Return a JSON object \
             with an \"entities\" key whose value is an array of detections.",
        );

        if !self.labels.is_empty() {
            let labels = self.labels.join(", ");
            prompt.push_str(&format!(
                "\n\nDocument context labels (adjust sensitivity to domain-specific \
                 visual content accordingly): {labels}."
            ));
        }

        if !self.hints.is_empty() {
            prompt.push_str(
                "\n\nThe uploader marked these regions as likely sensitive. \
                 Confirm or relocate each via your detections; ignore those you \
                 disagree with. Hints:",
            );
            for (i, h) in self.hints.iter().enumerate() {
                let bbox = &h.location.bounding_box;
                let kind = h
                    .entity_kind
                    .map(|k| k.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let name = h.name.as_deref().unwrap_or("");
                prompt.push_str(&format!(
                    "\n[hint {i}] name=\"{name}\", kind={kind}, \
                     bbox=({x}, {y}, {w}, {h})",
                    x = bbox.x,
                    y = bbox.y,
                    w = bbox.width,
                    h = bbox.height,
                ));
            }
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
