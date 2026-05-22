//! CV-specific prompt construction.
//!
//! [`CvPromptBuilder`] builds the user prompt that hands the VLM a
//! list of pre-computed CV detections (faces, plates, signatures
//! with bboxes) and asks it to classify each one into an entity
//! category. CV detection itself runs upstream of the pipeline.

use super::CvDetection;
use crate::agent::DetectionConfig;

/// Fallback when no specific entity types are requested.
const ALL_CV_TYPES_HINT: &str = "all detectable object types";

/// Builds user prompts for VLM-side classification of pre-computed
/// CV detections.
pub(crate) struct CvPromptBuilder<'a> {
    config: &'a DetectionConfig,
    detections: &'a [CvDetection],
}

impl<'a> CvPromptBuilder<'a> {
    /// Create a prompt builder from a [`DetectionConfig`] and the
    /// detections to classify.
    pub fn new(config: &'a DetectionConfig, detections: &'a [CvDetection]) -> Self {
        Self { config, detections }
    }

    /// Build the user prompt for the given base64-encoded image.
    pub fn build(&self, image_b64: &str) -> String {
        let entity_hint = if self.config.entity_kinds.is_empty() {
            ALL_CV_TYPES_HINT.to_string()
        } else {
            self.config
                .entity_kinds
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };

        let threshold_clause = match self.config.confidence_threshold {
            Some(t) => format!(" Drop detections below confidence {t:.2}."),
            None => String::new(),
        };

        let detections_json =
            serde_json::to_string(self.detections).unwrap_or_else(|_| "[]".to_string());

        format!(
            "Classify each pre-computed CV detection below into an entity \
             category and type from [{entity_hint}].{threshold_clause}\n\n\
             Detections (JSON): {detections_json}\n\n\
             Image (base64): {image_b64}",
        )
    }
}

/// Default system prompt for the CV agent.
pub(super) const CV_SYSTEM_PROMPT: &str = "\
You are a vision-language model classifying privacy-sensitive content in images. \
You are given a base64-encoded image and a JSON array of pre-computed CV detections \
(each with label, confidence, and bbox).\n\
\n\
Your task: classify each detection into an entity category and specific entity type, \
then return a JSON array of classified entities. Each entry has: \
category, entity_type, label, confidence, bbox ([x, y, width, height] in pixels).\n\
\n\
Common entity mappings:\n\
- face → category: biometric, entity_type: face\n\
- license_plate → category: personal_identity, entity_type: vehicle_registration\n\
- signature → category: biometric, entity_type: signature\n\
- handwriting → category: personal_identity, entity_type: person_name (if it contains a name)\n\
\n\
If no detections are provided, return an empty array [].";
