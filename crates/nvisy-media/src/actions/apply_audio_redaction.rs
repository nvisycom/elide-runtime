//! Placeholder audio redaction action.

use serde::Deserialize;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::error::{Error, ErrorKind};
use nvisy_core::registry::action::Action;

/// Typed parameters for [`ApplyAudioRedactionAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyAudioRedactionParams {
    /// Time segments to mute, as `(start_seconds, end_seconds)` pairs.
    #[serde(default)]
    pub mute_segments: Vec<(f64, f64)>,
}

/// Placeholder action for audio redaction.
///
/// Returns a runtime error indicating audio redaction is not yet implemented.
pub struct ApplyAudioRedactionAction;

#[async_trait::async_trait]
impl Action for ApplyAudioRedactionAction {
    type Params = ApplyAudioRedactionParams;

    fn id(&self) -> &str {
        "apply-audio-redaction"
    }

    fn validate_params(&self, _params: &Self::Params) -> Result<(), Error> {
        Ok(())
    }

    async fn execute(
        &self,
        mut input: mpsc::Receiver<Blob>,
        output: mpsc::Sender<Blob>,
        _params: Self::Params,
    ) -> Result<u64, Error> {
        // Pass through blobs unchanged — audio redaction is not implemented
        while let Some(blob) = input.recv().await {
            tracing::warn!("Audio redaction not yet implemented, passing through unchanged");
            if output.send(blob).await.is_err() {
                return Err(Error::new(
                    ErrorKind::Runtime,
                    "Audio redaction not yet implemented",
                ));
            }
        }
        Ok(0)
    }
}
