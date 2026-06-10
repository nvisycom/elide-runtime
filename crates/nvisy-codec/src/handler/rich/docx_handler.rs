//! DOCX handler (stub: text extraction awaiting implementation).
//!
//! Preserves the raw bytes for round-trip encoding and exposes no
//! text chunks until per-paragraph extraction lands.

use std::ops::Range;
use std::sync::Arc;

use bytes::Bytes;
use nvisy_core::Error;
use nvisy_core::modality::{Text, TextData, TextLocation};
use nvisy_core::redaction::Redactions;

use super::DocxLoader;
use crate::content::{ContentData, ContentSource};
use crate::core::{Chunk, Handle, Handler, ModalityKind};
use crate::handler::text::lift_identity;
use crate::{Format, FormatId, LoaderAdapter};

const TARGET: &str = "docx-handler";

/// Stable [`FormatId`] for the DOCX codec.
pub const FORMAT_ID: FormatId = FormatId::from_static("nvisy.rich.docx");

/// [`Format`] descriptor registered into [`crate::CodecRegistry`].
pub fn format() -> Format {
    Format {
        id: FORMAT_ID.clone(),
        modality: ModalityKind::Text,
        extensions: vec!["docx".into()],
        content_types: vec![
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into(),
        ],
        loader: Arc::new(LoaderAdapter::new(DocxLoader)),
    }
}

#[derive(Debug)]
pub struct DocxHandler {
    source: ContentSource,
    raw: Bytes,
}

impl DocxHandler {
    pub fn new(raw: impl Into<Bytes>) -> Self {
        Self {
            source: ContentSource::new(),
            raw: raw.into(),
        }
    }

    pub fn with_source(mut self, source: ContentSource) -> Self {
        self.source = source;
        self
    }

    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}

impl Handler for DocxHandler {
    fn format(&self) -> FormatId {
        FORMAT_ID.clone()
    }

    fn source(&self) -> &ContentSource {
        &self.source
    }

    #[tracing::instrument(name = "docx.encode", skip_all, fields(output_bytes))]
    fn encode(&self) -> Result<ContentData, Error> {
        tracing::Span::current().record("output_bytes", self.raw.len());
        let source = ContentSource::new().with_parent(&self.source);
        Ok(ContentData::new(source, self.raw.clone()))
    }
}

#[async_trait::async_trait]
impl Handle<Text> for DocxHandler {
    async fn next_chunk(&mut self) -> Result<Option<Chunk<Text>>, Error> {
        Ok(None)
    }

    fn lift_chunk(&self, chunk: &Chunk<Text>, value_range: Range<usize>) -> Option<TextLocation> {
        lift_identity(chunk, value_range)
    }

    async fn read(&self, _location: &TextLocation) -> Result<Option<TextData>, Error> {
        Ok(None)
    }

    async fn redact(&mut self, redactions: Redactions<Text>) -> Result<(), Error> {
        if redactions.is_empty() {
            return Ok(());
        }
        Err(Error::validation(
            "DOCX redaction is not yet supported",
            TARGET,
        ))
    }
}
