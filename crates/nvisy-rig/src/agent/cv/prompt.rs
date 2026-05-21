//! CV-specific prompt construction.
//!
//! [`CvPromptBuilder`] constructs the user prompt that instructs the VLM
//! to call the CV tool and classify detections into entity categories.

use crate::agent::DetectionConfig;

/// Fallback when no specific entity types are requested.
const ALL_CV_TYPES_HINT: &str = "all detectable object types";

/// Builds user prompts for CV-based object detection.
///
/// Encodes entity-kind filters and confidence thresholds into the prompt
/// alongside the base64-encoded image data.
pub(crate) struct CvPromptBuilder<'a> {
    config: &'a DetectionConfig,
}

impl<'a> CvPromptBuilder<'a> {
    /// Create a prompt builder from a [`DetectionConfig`].
    pub fn new(config: &'a DetectionConfig) -> Self {
        Self { config }
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
            Some(t) => format!(" with minimum confidence {t:.2}"),
            None => String::new(),
        };
        format!(
            "Detect objects of types [{entity_hint}]{threshold_clause} in the \
             following base64-encoded image using the cv_detect_objects \
             tool.\n\nImage (base64): {image_b64}",
        )
    }
}

/// Default system prompt for the CV agent.
pub(super) const CV_SYSTEM_PROMPT: &str = "\
You are a vision-language model performing object detection for privacy-sensitive content in images. \
You have access to a computer vision tool that detects faces, license plates, and signatures.\n\
\n\
Your workflow:\n\
1. Use the cv_detect_objects tool to detect objects in the provided image.\n\
2. Analyze the detections and classify each into an entity category and specific entity type.\n\
3. Return a JSON array of detected entities, each with keys: \
   category, entity_type, label, confidence, bbox ([x, y, width, height] in pixels).\n\
\n\
Common entity mappings:\n\
- face → category: biometric, entity_type: face\n\
- license_plate → category: personal_identity, entity_type: vehicle_registration\n\
- signature → category: biometric, entity_type: signature\n\
- handwriting → category: personal_identity, entity_type: person_name (if it contains a name)\n\
\n\
If no objects are detected, return an empty array [].";
