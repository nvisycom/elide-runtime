//! OCR-specific prompt construction.
//!
//! [`OcrPromptBuilder`] constructs the user prompt that instructs the VLM
//! to call the OCR tool and then detect entities in the extracted text.

use crate::backend::{DetectionConfig, ALL_TYPES_HINT};

/// Builds user prompts for OCR-based entity extraction.
///
/// Encodes entity-kind filters and confidence thresholds into the prompt
/// alongside the base64-encoded image data.
pub(crate) struct OcrPromptBuilder<'a> {
    config: &'a DetectionConfig,
}

impl<'a> OcrPromptBuilder<'a> {
    /// Create a prompt builder from a [`DetectionConfig`].
    pub fn new(config: &'a DetectionConfig) -> Self {
        Self { config }
    }

    /// Build the user prompt for the given base64-encoded image.
    pub fn build(&self, image_b64: &str) -> String {
        let entity_hint = if self.config.entity_kinds.is_empty() {
            ALL_TYPES_HINT.to_string()
        } else {
            self.config
                .entity_kinds
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };

        format!(
            "Extract text from the following base64-encoded image using the \
             ocr_extract_text tool, then detect entities of types [{entity_hint}] \
             with minimum confidence {threshold:.2}.\n\n\
             Image (base64): {image_b64}",
            threshold = self.config.confidence_threshold,
        )
    }
}

/// Default system prompt for the OCR agent.
pub(super) const OCR_SYSTEM_PROMPT: &str = "\
You are a vision-language model performing OCR and entity detection on images. \
You have access to an OCR tool that extracts text from images. \
\n\
Your workflow:\n\
1. Use the ocr_extract_text tool to extract all text from the provided image.\n\
2. Analyze the extracted text for personally identifiable information (PII), \
   protected health information (PHI), financial data, and credentials.\n\
3. Return a JSON object with two fields:\n\
   - \"extracted_text\": the full text extracted from the image\n\
   - \"entities\": a JSON array of detected entities, each with keys: \
     category, entity_type, value, confidence, bbox (optional [x, y, w, h] array)\n\
\n\
If no entities are found, return an empty array for \"entities\". \
If OCR produces no text, return an empty string for \"extracted_text\" and an empty array for \"entities\".";
