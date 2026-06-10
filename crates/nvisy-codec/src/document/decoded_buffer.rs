//! [`DecodedBuffer<M>`]: thin wrapper over a typed [`DocumentHandle<M>`]
//! that implements the core read/write trait surface
//! ([`TextAt`]/[`DataAt`]/[`RedactAt`]).
//!
//! The wrapper exists so any pipeline component can read from /
//! write to a codec-backed source through the same `*At` traits the
//! engine bounds on, without naming a concrete handler shape.
//!
//! Modality coverage:
//!
//! | Modality   | TextAt | DataAt | RedactAt |
//! |------------|--------|--------|----------|
//! | Text       |   ✓    |   ✓    |    ✓     |
//! | Tabular    |   ✓    |   ✓    |    ✓     |
//! | Image      |        |   ✓    |    ✓     |
//! | Audio      |        |   ✓    |    ✓     |
//!
//! Image and audio intentionally don't implement [`TextAt`]: "text
//! at this location" for image means OCR text and for audio means
//! transcript text — both produced by the extraction phase
//! (`nvisy-toolkit::extraction`) and stamped onto document blocks
//! (`nvisy-engine`). The codec layer has no visibility into either.
//!
//! [`DocumentHandle<M>`]: crate::document::DocumentHandle
//! [`TextAt`]: nvisy_core::extraction::TextAt
//! [`DataAt`]: nvisy_core::extraction::DataAt
//! [`RedactAt`]: nvisy_core::redaction::RedactAt

use nvisy_core::Result;
use nvisy_core::extraction::DataAt;
#[cfg(any(feature = "internal_text", feature = "internal_tabular"))]
use nvisy_core::extraction::TextAt;
use nvisy_core::redaction::{RedactAt, Redactions};

use crate::core::Codable;
use crate::document::DocumentHandle;

/// Format-aware buffer wrapping a typed [`DocumentHandle<M>`].
///
/// Construct via [`new`] from a typed handle the caller already
/// obtained from the registry — there are no path-based or
/// bytes-based constructors here: the registry is the front door.
///
/// [`new`]: Self::new
pub struct DecodedBuffer<M: Codable> {
    handle: DocumentHandle<M>,
}

impl<M: Codable> DecodedBuffer<M> {
    /// Wrap a typed handle. The caller is responsible for matching
    /// the modality variant out of [`UntypedDocumentHandle`] before
    /// calling here.
    ///
    /// [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle
    pub fn new(handle: DocumentHandle<M>) -> Self {
        Self { handle }
    }

    /// Borrow the inner handle.
    pub fn handle(&self) -> &DocumentHandle<M> {
        &self.handle
    }

    /// Borrow the inner handle mutably (for codec-level operations
    /// the trait surface doesn't expose).
    pub fn handle_mut(&mut self) -> &mut DocumentHandle<M> {
        &mut self.handle
    }

    /// Consume the buffer and return the inner handle.
    pub fn into_handle(self) -> DocumentHandle<M> {
        self.handle
    }
}

// ── Text ────────────────────────────────────────────────────────────

#[cfg(feature = "internal_text")]
#[async_trait::async_trait]
impl TextAt<nvisy_core::modality::Text> for DecodedBuffer<nvisy_core::modality::Text> {
    async fn text_at(&self, location: &nvisy_core::modality::TextLocation) -> Option<String> {
        self.handle
            .handler()
            .read(location)
            .await
            .ok()
            .flatten()
            .map(|d| d.into_string())
    }
}

#[cfg(feature = "internal_text")]
#[async_trait::async_trait]
impl DataAt<nvisy_core::modality::Text> for DecodedBuffer<nvisy_core::modality::Text> {
    async fn data_at(
        &self,
        location: &nvisy_core::modality::TextLocation,
    ) -> Option<nvisy_core::modality::TextData> {
        self.handle.handler().read(location).await.ok().flatten()
    }
}

#[cfg(feature = "internal_text")]
#[async_trait::async_trait]
impl RedactAt<nvisy_core::modality::Text> for DecodedBuffer<nvisy_core::modality::Text> {
    async fn redact_at(
        &mut self,
        redactions: Redactions<nvisy_core::modality::Text>,
    ) -> Result<()> {
        self.handle.handler_mut().redact(redactions).await
    }
}

// ── Tabular ─────────────────────────────────────────────────────────

#[cfg(feature = "internal_tabular")]
#[async_trait::async_trait]
impl TextAt<nvisy_core::modality::Tabular> for DecodedBuffer<nvisy_core::modality::Tabular> {
    async fn text_at(&self, location: &nvisy_core::modality::TabularLocation) -> Option<String> {
        self.handle
            .handler()
            .read(location)
            .await
            .ok()
            .flatten()
            .map(|d| d.into_string())
    }
}

#[cfg(feature = "internal_tabular")]
#[async_trait::async_trait]
impl DataAt<nvisy_core::modality::Tabular> for DecodedBuffer<nvisy_core::modality::Tabular> {
    async fn data_at(
        &self,
        location: &nvisy_core::modality::TabularLocation,
    ) -> Option<nvisy_core::modality::TextData> {
        self.handle.handler().read(location).await.ok().flatten()
    }
}

#[cfg(feature = "internal_tabular")]
#[async_trait::async_trait]
impl RedactAt<nvisy_core::modality::Tabular> for DecodedBuffer<nvisy_core::modality::Tabular> {
    async fn redact_at(
        &mut self,
        redactions: Redactions<nvisy_core::modality::Tabular>,
    ) -> Result<()> {
        self.handle.handler_mut().redact(redactions).await
    }
}

// ── Image ───────────────────────────────────────────────────────────

#[cfg(feature = "internal_image")]
#[async_trait::async_trait]
impl DataAt<nvisy_core::modality::Image> for DecodedBuffer<nvisy_core::modality::Image> {
    async fn data_at(
        &self,
        location: &nvisy_core::modality::ImageLocation,
    ) -> Option<nvisy_core::modality::ImageData> {
        self.handle.handler().read(location).await.ok().flatten()
    }
}

#[cfg(feature = "internal_image")]
#[async_trait::async_trait]
impl RedactAt<nvisy_core::modality::Image> for DecodedBuffer<nvisy_core::modality::Image> {
    async fn redact_at(
        &mut self,
        redactions: Redactions<nvisy_core::modality::Image>,
    ) -> Result<()> {
        self.handle.handler_mut().redact(redactions).await
    }
}

// ── Audio ───────────────────────────────────────────────────────────

#[cfg(feature = "internal_audio")]
#[async_trait::async_trait]
impl DataAt<nvisy_core::modality::Audio> for DecodedBuffer<nvisy_core::modality::Audio> {
    async fn data_at(
        &self,
        location: &nvisy_core::modality::AudioLocation,
    ) -> Option<nvisy_core::modality::AudioData> {
        self.handle.handler().read(location).await.ok().flatten()
    }
}

#[cfg(feature = "internal_audio")]
#[async_trait::async_trait]
impl RedactAt<nvisy_core::modality::Audio> for DecodedBuffer<nvisy_core::modality::Audio> {
    async fn redact_at(
        &mut self,
        redactions: Redactions<nvisy_core::modality::Audio>,
    ) -> Result<()> {
        self.handle.handler_mut().redact(redactions).await
    }
}
