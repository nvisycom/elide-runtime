//! Modality-specific codec accessors on [`DocumentEnvelope`].
//!
//! Thin forwarders to the codec [`DocumentHandle`] for the per-modality
//! `locations`/`read`/`redact` operations, gated by the same features
//! as the codec handler variants.

use futures::StreamExt;
use nvisy_codec::Span;
use nvisy_codec::{Located, handler};
use nvisy_core::Error;
use nvisy_ontology::modality::{Audio, Image, Tabular, Text};

use super::DocumentEnvelope;

impl DocumentEnvelope {
    /// Collect all text locations from the codec handle into a `Vec`.
    pub async fn collect_text_locations(&self) -> Vec<Located<Text>> {
        self.handle.text_locations().collect().await
    }

    /// Read the text content at the given text location from the
    /// codec handle.
    pub async fn read_text(&self, location: &Text) -> Option<handler::TextData> {
        self.handle.read_text(location).await
    }

    /// Collect every text location together with its data, skipping
    /// locations the handle can't read. Used by detection ops that
    /// scan extracted text spans without caring about the underlying
    /// streaming machinery.
    pub async fn collect_text_spans(&self) -> Vec<Span<Text, handler::TextData>> {
        let locations = self.collect_text_locations().await;
        let mut spans = Vec::with_capacity(locations.len());
        for located in locations {
            if let Some(data) = self.read_text(&located.location).await {
                spans.push(Span::from_located(located, data));
            }
        }
        spans
    }

    /// Apply a batch of text redactions to the codec handle.
    pub async fn apply_text_redactions(
        &mut self,
        redactions: handler::Redactions<Text, handler::TextRedaction>,
    ) -> Result<(), Error> {
        self.handle.apply_text_redactions(redactions).await
    }
}

#[cfg(feature = "image")]
impl DocumentEnvelope {
    /// Collect all image locations from the codec handle into a `Vec`.
    pub async fn collect_image_locations(&self) -> Vec<Located<Image>> {
        self.handle.image_locations().collect().await
    }

    /// Read the image data at the given image location.
    pub async fn read_image(&self, location: &Image) -> Option<handler::ImageData> {
        self.handle.read_image(location).await
    }

    /// Apply a batch of image redactions to the codec handle.
    pub async fn apply_image_redactions(
        &mut self,
        redactions: handler::Redactions<Image, handler::ImageRedaction>,
    ) -> Result<(), Error> {
        self.handle.apply_image_redactions(redactions).await
    }
}

#[cfg(feature = "audio")]
impl DocumentEnvelope {
    /// Collect all audio locations from the codec handle into a `Vec`.
    pub async fn collect_audio_locations(&self) -> Vec<Located<Audio>> {
        self.handle.audio_locations().collect().await
    }

    /// Read the audio data at the given audio location.
    pub async fn read_audio(&self, location: &Audio) -> Option<handler::AudioData> {
        self.handle.read_audio(location).await
    }

    /// Apply a batch of audio redactions to the codec handle.
    pub async fn apply_audio_redactions(
        &mut self,
        redactions: handler::Redactions<Audio, handler::AudioRedaction>,
    ) -> Result<(), Error> {
        self.handle.apply_audio_redactions(redactions).await
    }
}

#[cfg(feature = "tabular")]
impl DocumentEnvelope {
    /// Collect all tabular (cell) locations from the codec handle.
    pub async fn collect_tabular_locations(&self) -> Vec<Located<Tabular>> {
        self.handle.tabular_locations().collect().await
    }

    /// Read the cell value at the given tabular location.
    pub async fn read_tabular(&self, location: &Tabular) -> Option<handler::TextData> {
        self.handle.read_tabular(location).await
    }

    /// Apply a batch of tabular redactions to the codec handle.
    pub async fn apply_tabular_redactions(
        &mut self,
        redactions: handler::Redactions<Tabular, handler::TabularRedaction>,
    ) -> Result<(), Error> {
        self.handle.apply_tabular_redactions(redactions).await
    }
}
