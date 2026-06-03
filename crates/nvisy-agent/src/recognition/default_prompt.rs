//! [`DefaultPrompt`]: the shipped [`Prompt`] impl, covering both
//! [`Text`] and [`Image`].
//!
//! Both impls follow the same pattern: build a structured-output
//! prompt with shared system instructions, ask the model to return
//! `{ "entities": [...] }`, deserialise into a candidate vec, and
//! lift each candidate into an `Entity<M>`. For text, the
//! candidate's `context` field is used to localize the value back
//! into a byte range; for image, the bounding box arrives in
//! normalised `[0, 1]` coordinates and is scaled to pixel space.

use std::sync::OnceLock;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nvisy_core::RecognizerInput;
use nvisy_core::entity::{Entity, ModelProvenance, TrailProvenance, TrailStep};
use nvisy_core::modality::{Image, Text};
use nvisy_core::primitive::Confidence;
use schemars::Schema;

use super::candidates::{TextCandidates, VlmCandidates};
use super::localize::{UnresolvedCandidatePolicy, localize_all};
use super::prompt::Prompt;
use super::response_parser::parse_json;
use super::text_prompt::TextPromptBuilder;
use super::vlm_prompt::VlmPromptBuilder;
use crate::backend::LlmResponse;

/// Default confidence assigned to a candidate when the LLM didn't
/// score it.
const DEFAULT_CONFIDENCE: f64 = 0.5;

/// Shipped [`Prompt`] impl covering both [`Text`] and [`Image`].
///
/// Stateless zero-sized type. Customise behaviour by writing your
/// own [`Prompt<M>`] impl rather than tweaking this one.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultPrompt;

fn text_schema() -> &'static Schema {
    static CACHE: OnceLock<Schema> = OnceLock::new();
    CACHE.get_or_init(|| schemars::schema_for!(TextCandidates))
}

fn vlm_schema() -> &'static Schema {
    static CACHE: OnceLock<Schema> = OnceLock::new();
    CACHE.get_or_init(|| schemars::schema_for!(VlmCandidates))
}

impl Prompt<Text> for DefaultPrompt {
    fn build(&self, input: &RecognizerInput<Text>) -> String {
        TextPromptBuilder::new(input.data.text.as_str(), &input.hints, &input.labels).build()
    }

    fn schema(&self) -> Option<&Schema> {
        Some(text_schema())
    }

    fn lift(&self, response: &LlmResponse, input: &RecognizerInput<Text>) -> Vec<Entity<Text>> {
        let Ok(parsed): Result<TextCandidates, _> = parse_json(&response.text) else {
            return Vec::new();
        };
        let text = input.data.text.as_str();
        let localized = localize_all(text, parsed.entities, UnresolvedCandidatePolicy::default());
        let model = ModelProvenance::new("llm".to_owned());

        let mut out = Vec::with_capacity(localized.len());
        for l in localized {
            let Some(entity_kind) = l.candidate.entity_type else {
                continue;
            };
            let raw = l.candidate.confidence.unwrap_or(DEFAULT_CONFIDENCE);
            let Some(confidence) = Confidence::new(raw.clamp(0.0, 1.0)) else {
                continue;
            };
            let location = Text::new(l.start_offset, l.end_offset);
            let reason = format!("llm identified {entity_kind}");
            let step = TrailStep::recognition(
                "llm-ner",
                confidence,
                TrailProvenance::Model(model.clone()),
                reason,
            );

            let mut b = Entity::builder()
                .with_entity_kind(entity_kind)
                .with_trail(vec![step])
                .with_confidence(confidence)
                .with_location(location);
            if let Some(id) = l.candidate.entity_id {
                b = b.with_entity_id(id);
            }
            out.push(b.build().expect("required fields provided"));
        }
        out
    }
}

impl Prompt<Image> for DefaultPrompt {
    fn build(&self, input: &RecognizerInput<Image>) -> String {
        let image_b64 = STANDARD.encode(input.data.bytes.as_ref());
        VlmPromptBuilder::new(&input.hints, &input.labels).build(&image_b64)
    }

    fn schema(&self) -> Option<&Schema> {
        Some(vlm_schema())
    }

    fn lift(&self, response: &LlmResponse, input: &RecognizerInput<Image>) -> Vec<Entity<Image>> {
        let Ok(parsed): Result<VlmCandidates, _> = parse_json(&response.text) else {
            return Vec::new();
        };
        let dims = input.data.dims;
        let model = ModelProvenance::new("llm".to_owned());

        let mut out = Vec::with_capacity(parsed.entities.len());
        for d in parsed.entities {
            let raw = d.confidence.unwrap_or(DEFAULT_CONFIDENCE);
            let Some(confidence) = Confidence::new(raw.clamp(0.0, 1.0)) else {
                continue;
            };
            let bbox = d.bbox.to_pixel(dims);
            let location = Image::new(bbox);
            let reason = format!("vlm identified {}", d.entity_kind);
            let step = TrailStep::recognition(
                "llm-vlm",
                confidence,
                TrailProvenance::Model(model.clone()),
                reason,
            );
            let entity = Entity::builder()
                .with_entity_kind(d.entity_kind)
                .with_trail(vec![step])
                .with_confidence(confidence)
                .with_location(location)
                .build()
                .expect("required fields provided");
            out.push(entity);
        }
        out
    }
}
