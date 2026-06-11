//! [`CodecRegistry`]: resolves an extension or content type to a
//! registered [`Format`] and decodes content through its loader.
//!
//! Downstream crates register their own formats by calling
//! [`CodecRegistry::add_format`] — there is no central enum to
//! extend.

use std::collections::HashMap;

use nvisy_core::Error;

use super::{Format, FormatId};
use crate::content::ContentData;
use crate::document::UntypedDocumentHandle;

/// Codec registry — owns the set of registered [`Format`]s and
/// resolves them by extension, content type, or id.
#[derive(Debug, Default)]
pub struct CodecRegistry {
    formats: Vec<Format>,
    by_id: HashMap<FormatId, usize>,
    by_extension: HashMap<String, usize>,
    by_content_type: HashMap<String, usize>,
}

impl CodecRegistry {
    /// Empty registry. Use [`with_format`] / [`add_format`] to add
    /// custom formats, or [`with_builtin`] to start from a pre-
    /// populated set of every built-in format the active feature
    /// set enables.
    ///
    /// [`with_format`]: Self::with_format
    /// [`add_format`]: Self::add_format
    /// [`with_builtin`]: Self::with_builtin
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-populated registry containing every built-in format the
    /// active feature set enables (TXT, JSON, HTML, CSV, PNG, JPEG,
    /// WAV, PDF, …). Equivalent to [`new`] followed by registering
    /// each built-in format.
    ///
    /// Add custom formats afterward with [`with_format`] (chainable)
    /// or [`add_format`] (in-place); they take precedence on
    /// extension / content-type collisions (last registration wins).
    ///
    /// [`new`]: Self::new
    /// [`with_format`]: Self::with_format
    /// [`add_format`]: Self::add_format
    pub fn with_builtin() -> Self {
        let mut registry = Self::new();
        #[cfg(feature = "txt")]
        registry.add_format(crate::handler::text::txt_format());
        #[cfg(feature = "json")]
        registry.add_format(crate::handler::text::json_format());
        #[cfg(feature = "markdown")]
        registry.add_format(crate::handler::text::markdown_format());
        #[cfg(feature = "html")]
        registry.add_format(crate::handler::text::html_format());
        #[cfg(feature = "csv")]
        registry.add_format(crate::handler::tabular::csv_format());
        #[cfg(feature = "xlsx")]
        registry.add_format(crate::handler::tabular::xlsx_format());
        #[cfg(feature = "png")]
        registry.add_format(crate::handler::image::png_format());
        #[cfg(feature = "jpeg")]
        registry.add_format(crate::handler::image::jpeg_format());
        #[cfg(feature = "tiff")]
        registry.add_format(crate::handler::image::tiff_format());
        #[cfg(feature = "wav")]
        registry.add_format(crate::handler::audio::wav_format());
        #[cfg(feature = "mp3")]
        registry.add_format(crate::handler::audio::mp3_format());
        #[cfg(feature = "pdf")]
        registry.add_format(crate::handler::rich::pdf_format());
        #[cfg(feature = "docx")]
        registry.add_format(crate::handler::rich::docx_format());
        registry
    }

    /// Register a [`Format`] and return `self` for chained builder
    /// calls. Delegates to [`add_format`] for the indexing body.
    ///
    /// # Panics
    ///
    /// Panics if the format's id is already registered. Extensions
    /// and content types that conflict with an existing format are
    /// overwritten (last registration wins) — register custom
    /// formats *after* [`with_builtin`] if you want them to take
    /// precedence.
    ///
    /// [`with_builtin`]: Self::with_builtin
    /// [`add_format`]: Self::add_format
    #[must_use]
    pub fn with_format(mut self, format: Format) -> Self {
        self.add_format(format);
        self
    }

    /// In-place equivalent of [`with_format`]. Useful with an
    /// already-mut binding (e.g. inside a cfg-stanza in
    /// [`with_builtin`]) where the `let registry = registry.with_format(...)`
    /// dance is just noise.
    ///
    /// # Panics
    ///
    /// Same conditions as [`with_format`].
    ///
    /// [`with_format`]: Self::with_format
    /// [`with_builtin`]: Self::with_builtin
    pub fn add_format(&mut self, format: Format) -> &mut Self {
        assert!(
            !self.by_id.contains_key(&format.id),
            "format id already registered: {}",
            format.id
        );
        let index = self.formats.len();
        for ext in &format.extensions {
            self.by_extension.insert(ext.to_ascii_lowercase(), index);
        }
        for ct in &format.content_types {
            self.by_content_type.insert(ct.to_ascii_lowercase(), index);
        }
        self.by_id.insert(format.id.clone(), index);
        self.formats.push(format);
        self
    }

    /// Look up a registered format by id.
    pub fn by_id(&self, id: &FormatId) -> Option<&Format> {
        self.by_id.get(id).map(|&i| &self.formats[i])
    }

    /// Look up a registered format by file extension
    /// (case-insensitive, no leading dot).
    pub fn by_extension(&self, ext: &str) -> Option<&Format> {
        self.by_extension
            .get(&ext.to_ascii_lowercase())
            .map(|&i| &self.formats[i])
    }

    /// Look up a registered format by MIME content type
    /// (case-insensitive).
    pub fn by_content_type(&self, mime: &str) -> Option<&Format> {
        self.by_content_type
            .get(&mime.to_ascii_lowercase())
            .map(|&i| &self.formats[i])
    }

    /// Iterate over every registered format in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &Format> {
        self.formats.iter()
    }

    /// Decode raw content using the format resolved from the
    /// extension hint. Accepts anything convertible into
    /// [`ContentData`] — `&str`, `&[u8]`, `Vec<u8>`, `Bytes`,
    /// `String`.
    pub async fn decode(
        &self,
        content: impl Into<ContentData>,
        extension: &str,
    ) -> Result<UntypedDocumentHandle, Error> {
        let format = self.by_extension(extension).ok_or_else(|| {
            Error::validation(
                format!("no codec registered for extension `{extension}`"),
                "nvisy_codec::handler::registry::decode",
            )
        })?;
        format.loader.decode(content.into()).await
    }
}
