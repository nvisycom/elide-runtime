//! Transcription result parsing.

use serde_json::Value;

use nvisy_core::math::TimeSpan;
use nvisy_core::Error;
use nvisy_ontology::entity::{DetectionMethod, Entity, EntityCategory, EntityKind};
use nvisy_ontology::location::{AudioLocation, Location};

/// Parse raw JSON dicts from a transcription backend into [`Entity`] values.
///
/// Expected dict keys: `text`, `start_time`, `end_time`, `confidence`,
/// and optionally `speaker_id`.
pub fn parse_transcribe_entities(raw: &[Value]) -> Result<Vec<Entity>, Error> {
    let mut entities = Vec::new();

    for item in raw {
        let obj = item.as_object().ok_or_else(|| {
            Error::python("Expected JSON object in transcription results".to_string())
        })?;

        let text = obj
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::python("Missing 'text' in transcription result".to_string()))?;

        let start_time = obj
            .get("start_time")
            .and_then(Value::as_f64)
            .ok_or_else(|| Error::python("Missing 'start_time'".to_string()))?;

        let end_time = obj
            .get("end_time")
            .and_then(Value::as_f64)
            .ok_or_else(|| Error::python("Missing 'end_time'".to_string()))?;

        let confidence = obj
            .get("confidence")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);

        let speaker_id = obj
            .get("speaker_id")
            .and_then(Value::as_str)
            .map(String::from);

        let entity = Entity::new(
            EntityCategory::Pii,
            EntityKind::PersonName,
            text,
            DetectionMethod::SpeechTranscript,
            confidence,
        )
        .with_location(Location::Audio(AudioLocation {
            time_span: TimeSpan {
                start_secs: start_time,
                end_secs: end_time,
            },
            speaker_id,
            audio_id: None,
        }));

        entities.push(entity);
    }

    Ok(entities)
}
