//! Audio accessors for [`Document`].

use futures::StreamExt;
use nvisy_codec::Located;
use nvisy_codec::handler::{AudioData, AudioRedaction, Redactions};
use nvisy_core::Error;
use nvisy_ontology::entity::AudioLocation;

use super::Document;

impl Document {
    /// Collect all audio locations into a `Vec`.
    pub async fn collect_audio_locations(&self) -> Vec<Located<AudioLocation>> {
        self.handle.audio_locations().collect().await
    }

    /// Read the audio data at the given audio location.
    pub async fn read_audio(&self, location: &AudioLocation) -> Option<AudioData> {
        self.handle.read_audio(location).await
    }

    /// Apply a batch of audio redactions to the document.
    pub async fn apply_audio_redactions(
        &mut self,
        redactions: Redactions<AudioLocation, AudioRedaction>,
    ) -> Result<(), Error> {
        self.handle.apply_audio_redactions(redactions).await
    }
}
