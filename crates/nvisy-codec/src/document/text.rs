//! [`DocumentHandle<Text>`] implements [`TextAt`], [`DataAt`], and
//! [`RedactAt`] for the [`Text`] modality by delegating to the
//! underlying [`Handle<Text>`].
//!
//! [`DocumentHandle<Text>`]: crate::document::DocumentHandle
//! [`Handle<Text>`]: crate::core::Handle
//! [`TextAt`]: nvisy_core::extraction::TextAt
//! [`DataAt`]: nvisy_core::extraction::DataAt
//! [`RedactAt`]: nvisy_core::redaction::RedactAt
//! [`Text`]: nvisy_core::modality::Text

use nvisy_core::Result;
use nvisy_core::extraction::{DataAt, TextAt};
use nvisy_core::modality::{Text, TextData, TextLocation};
use nvisy_core::redaction::{RedactAt, Redactions};

use crate::document::DocumentHandle;

#[async_trait::async_trait]
impl TextAt<Text> for DocumentHandle<Text> {
    async fn text_at(&self, location: &TextLocation) -> Option<String> {
        self.handler()
            .read(location)
            .await
            .ok()
            .flatten()
            .map(|d| d.into_string())
    }
}

#[async_trait::async_trait]
impl DataAt<Text> for DocumentHandle<Text> {
    async fn data_at(&self, location: &TextLocation) -> Option<TextData> {
        self.handler().read(location).await.ok().flatten()
    }
}

#[async_trait::async_trait]
impl RedactAt<Text> for DocumentHandle<Text> {
    async fn redact_at(&mut self, redactions: Redactions<Text>) -> Result<()> {
        self.handler_mut().redact(redactions).await
    }
}
