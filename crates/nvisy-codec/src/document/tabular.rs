//! [`DocumentHandle<Tabular>`] implements [`TextAt`], [`DataAt`],
//! and [`RedactAt`] for the [`Tabular`] modality by delegating to
//! the underlying [`Handle<Tabular>`].
//!
//! [`DocumentHandle<Tabular>`]: crate::document::DocumentHandle
//! [`Handle<Tabular>`]: crate::core::Handle
//! [`TextAt`]: nvisy_core::extraction::TextAt
//! [`DataAt`]: nvisy_core::extraction::DataAt
//! [`RedactAt`]: nvisy_core::redaction::RedactAt
//! [`Tabular`]: nvisy_core::modality::Tabular

use nvisy_core::Result;
use nvisy_core::extraction::{DataAt, TextAt};
use nvisy_core::modality::{Tabular, TabularLocation, TextData};
use nvisy_core::redaction::{RedactAt, Redactions};

use crate::document::DocumentHandle;

#[async_trait::async_trait]
impl TextAt<Tabular> for DocumentHandle<Tabular> {
    async fn text_at(&self, location: &TabularLocation) -> Option<String> {
        self.handler()
            .read(location)
            .await
            .ok()
            .flatten()
            .map(|d| d.into_string())
    }
}

#[async_trait::async_trait]
impl DataAt<Tabular> for DocumentHandle<Tabular> {
    async fn data_at(&self, location: &TabularLocation) -> Option<TextData> {
        self.handler().read(location).await.ok().flatten()
    }
}

#[async_trait::async_trait]
impl RedactAt<Tabular> for DocumentHandle<Tabular> {
    async fn redact_at(&mut self, redactions: Redactions<Tabular>) -> Result<()> {
        self.handler_mut().redact(redactions).await
    }
}
