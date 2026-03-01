//! Audio document redaction.
//!
//! Maps [`AudioRedactionInput`] variants to codec [`AudioRedactionOutput`]
//! values and applies them to audio documents via time-range redaction.

use std::collections::HashMap;
use uuid::Uuid;

use nvisy_codec::document::Document;
use nvisy_codec::handler::WavHandler;
use nvisy_codec::transform::{AudioHandler, AudioRedaction, AudioRedactionOutput};
use nvisy_core::Error;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::location::Location;
use nvisy_ontology::record::Redaction;
use nvisy_ontology::specification::{AudioRedactionInput, RedactionInput};

/// Convert a [`RedactionInput::Audio`] into a codec [`AudioRedactionOutput`].
pub(crate) fn audio_output_from_spec(spec: &RedactionInput) -> Option<AudioRedactionOutput> {
    match spec {
        RedactionInput::Audio(audio) => Some(match audio {
            AudioRedactionInput::Silence => AudioRedactionOutput::Silence,
            AudioRedactionInput::Remove => AudioRedactionOutput::Remove,
            AudioRedactionInput::Synthesize => {
                // Synthesize is not yet supported; fall back to silence.
                AudioRedactionOutput::Silence
            }
        }),
        _ => None,
    }
}

pub(crate) async fn apply_audio_doc(
    doc: &Document<WavHandler>,
    entity_map: &HashMap<Uuid, &Entity>,
    redaction_map: &HashMap<Uuid, &Redaction>,
) -> Result<Document<WavHandler>, Error> {
    let mut redactions: Vec<AudioRedaction> = Vec::new();

    for (&entity_id, redaction) in redaction_map {
        let entity = match entity_map.get(&entity_id) {
            Some(e) => e,
            None => continue,
        };

        let audio_loc = match &entity.location {
            Some(Location::Audio(loc)) => loc,
            _ => continue,
        };

        let output = match audio_output_from_spec(&redaction.spec) {
            Some(o) => o,
            None => continue,
        };

        redactions.push(AudioRedaction {
            start_secs: audio_loc.time_span.start_secs,
            end_secs: audio_loc.time_span.end_secs,
            output,
        });
    }

    if redactions.is_empty() {
        return Ok(doc.clone());
    }

    let mut result = doc.clone();
    result.handler_mut().redact_spans(&redactions).await?;
    result.source.set_parent_id(Some(doc.source.as_uuid()));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nvisy_ontology::specification::TextRedactionInput;

    #[test]
    fn audio_output_silence() {
        let spec = RedactionInput::Audio(AudioRedactionInput::Silence);
        assert_eq!(audio_output_from_spec(&spec), Some(AudioRedactionOutput::Silence));
    }

    #[test]
    fn audio_output_remove() {
        let spec = RedactionInput::Audio(AudioRedactionInput::Remove);
        assert_eq!(audio_output_from_spec(&spec), Some(AudioRedactionOutput::Remove));
    }

    #[test]
    fn audio_output_synthesize_falls_back_to_silence() {
        let spec = RedactionInput::Audio(AudioRedactionInput::Synthesize);
        assert_eq!(audio_output_from_spec(&spec), Some(AudioRedactionOutput::Silence));
    }

    #[test]
    fn audio_output_text_spec_returns_none() {
        let spec = RedactionInput::Text(TextRedactionInput::Remove);
        assert_eq!(audio_output_from_spec(&spec), None);
    }
}
