//! Audio document redaction (stub).

use std::collections::HashMap;
use uuid::Uuid;

use nvisy_codec::handler::WavHandler;
use nvisy_codec::document::Document;
use nvisy_detection::{Entity, Redaction};
use nvisy_core::Error;

pub(crate) async fn apply_audio_doc(
    doc: &Document<WavHandler>,
    _entity_map: &HashMap<Uuid, &Entity>,
    _redaction_map: &HashMap<Uuid, &Redaction>,
) -> Result<Document<WavHandler>, Error> {
    tracing::warn!("audio redaction not yet implemented");
    Ok(doc.clone())
}
