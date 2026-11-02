//! Turning a [`Document`]'s bytes into content the engine can
//! address, under whatever codec settings the request asked for.
//!
//! Two paths, and which one runs is the whole point of this
//! module. A request that configures nothing decodes through the
//! registry the [`Provider`] built once at construction. A request
//! that overrides a format's default gets a registry assembled for
//! it, with each configured format replaced and every other codec
//! taken from the built-in set. The hidden-content policy goes to
//! the decode itself on both paths.
//!
//! The assembly is deliberately not a method on [`Engine`]: it
//! reads [`CodecParams`] and produces a [`FormatRegistry`], and
//! touches no engine state on the way. Keeping it free-standing is
//! what makes that testable and what keeps the per-codec `cfg`s
//! from spreading into the verbs.
//!
//! [`Document`]: crate::file::Document
//! [`Engine`]: super::Engine
//! [`Provider`]: elide_provider::Provider

use bytes::Bytes;
use elide::codec::{Document as CodecDocument, FormatRegistry};
use elide::{Document as EngineDocument, Error, ErrorKind, Result};
use elide_provider::{CodecParams, RasterMode};

use super::Engine;
use crate::file::Document;

impl Engine {
    /// Turn a document's bytes into content the engine can address.
    ///
    /// Resolves the format from the document's extension, then
    /// decodes under `codec` — sharing the prebuilt registry when
    /// every format is at its default, rebuilding one when not.
    pub(super) async fn decode(
        &self,
        document: Document,
        codec: CodecParams,
    ) -> Result<EngineDocument> {
        // The format comes from the document's name unless it
        // carries an explicit override; neither means there is
        // nothing to resolve a codec from.
        let extension = document.resolved_extension().ok_or_else(|| {
            Error::new(
                ErrorKind::MalformedInput,
                format!(
                    "document `{}` has no extension to resolve a format from: give the \
                     name one, or set it explicitly with `Document::with_extension`.",
                    document.name,
                ),
            )
        })?;
        let Document { name, bytes, .. } = document;
        // The shared registry is built for the default formats, so
        // only a request that overrides one has to pay for a rebuild.
        let result = if codec.uses_default_formats() {
            self.provider()
                .formats()
                .decode(bytes, &extension, codec.hidden_content)
                .await
        } else {
            decode_configured(bytes, &extension, codec).await
        };
        let handle = result.map_err(|err| {
            // A missing renderer is not malformed input; keep the kind so
            // callers can tell "unsupported build" from "bad document".
            let kind = match err.kind() {
                ErrorKind::CapabilityUnavailable => ErrorKind::CapabilityUnavailable,
                _ => ErrorKind::MalformedInput,
            };
            Error::new(
                kind,
                format!("codec decode failed for extension {extension:?}: {err}"),
            )
        })?;
        // The name roots every part path the report reports, so it
        // is attached here rather than left to the codec: a handle
        // is bytes and format, never an identity.
        Ok(EngineDocument::new(name.as_str().to_owned(), handle))
    }
}

/// Slow-path decode for a request overriding a format's default.
///
/// Rebuilds a [`FormatRegistry`] with each configured format
/// replaced; every other codec comes from the built-in set. One
/// registry build per non-default request: trivial next to the
/// work it configures (`Always { dpi }` pages a whole document
/// at the chosen DPI), but not free, so the default path skips
/// it entirely.
async fn decode_configured(
    bytes: Bytes,
    extension: &str,
    codec: CodecParams,
) -> std::result::Result<CodecDocument, Error> {
    // Unused when no format with a configurable default is built.
    #[allow(unused_mut)]
    let mut registry = FormatRegistry::with_builtin();

    // A raster mode other than `Auto` cannot be honoured without
    // the renderer. Refuse rather than decode without it:
    // silently falling back to `Auto` would hand a text-layer
    // extraction to a caller who asked to rasterize.
    if codec.raster_mode != RasterMode::default() {
        #[cfg(feature = "codec-pdf-render")]
        {
            registry = registry
                .with_replaced_format(elide::codec::pdf::pdf_format_with(codec.raster_mode));
        }
        #[cfg(not(feature = "codec-pdf-render"))]
        return Err(Error::new(
            ErrorKind::CapabilityUnavailable,
            format!(
                "raster mode {:?} requires the `codec-pdf-render` feature; \
                 rebuild with it enabled or request the default mode",
                codec.raster_mode
            ),
        ));
    }

    #[cfg(feature = "codec-csv")]
    if codec.csv_has_headers != CodecParams::default().csv_has_headers
        || codec.csv_delimiter.is_some()
    {
        registry = registry.with_replaced_format(elide::codec::plain::csv_format_with(
            codec.csv_has_headers,
            codec.csv_delimiter,
        ));
    }

    registry
        .decode(bytes, extension, codec.hidden_content)
        .await
}
