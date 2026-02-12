//! Placeholder audio redaction action.

use serde::Deserialize;

use nvisy_core::error::Error;
use nvisy_core::io::ContentData;

use crate::action::Action;

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
/// Passes through content unchanged -- audio redaction is not yet implemented.
pub struct ApplyAudioRedactionAction {
    params: ApplyAudioRedactionParams,
}

#[async_trait::async_trait]
impl Action for ApplyAudioRedactionAction {
    type Params = ApplyAudioRedactionParams;
    type Input = ContentData;
    type Output = ContentData;

    fn id(&self) -> &str {
        "apply-audio-redaction"
    }

    async fn connect(params: Self::Params) -> Result<Self, Error> {
        Ok(Self { params })
    }

    async fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Self::Output, Error> {
        tracing::warn!("Audio redaction not yet implemented, passing through unchanged");
        Ok(input)
    }
}
