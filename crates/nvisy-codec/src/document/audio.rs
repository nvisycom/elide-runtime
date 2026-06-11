//! [`DocumentHandle<Audio>`] implements [`DataAt`] and
//! [`RedactAt`] for the [`Audio`] modality by delegating to the
//! underlying [`Handler<Audio>`].
//!
//! Audio does not implement [`TextAt`]: "text at this location" for
//! audio is transcript text, which the extraction phase (in
//! `nvisy-toolkit::extraction`) produces and the engine stamps onto
//! document blocks. The codec layer has no visibility into STT.
//!
//! [`DocumentHandle<Audio>`]: crate::document::DocumentHandle
//! [`Handler<Audio>`]: crate::core::Handler
//! [`TextAt`]: nvisy_core::extraction::TextAt
//! [`DataAt`]: nvisy_core::extraction::DataAt
//! [`RedactAt`]: nvisy_core::redaction::RedactAt
//! [`Audio`]: nvisy_core::modality::Audio

use nvisy_core::Result;
use nvisy_core::extraction::DataAt;
use nvisy_core::modality::{Audio, AudioData, AudioLocation};
use nvisy_core::redaction::{RedactAt, Redactions};

use crate::document::DocumentHandle;

#[async_trait::async_trait]
impl DataAt<Audio> for DocumentHandle<Audio> {
    async fn data_at(&self, location: &AudioLocation) -> Option<AudioData> {
        self.handler().read(location).await.ok().flatten()
    }
}

#[async_trait::async_trait]
impl RedactAt<Audio> for DocumentHandle<Audio> {
    async fn redact_at(&mut self, redactions: Redactions<Audio>) -> Result<()> {
        self.handler_mut().redact(redactions).await
    }
}
