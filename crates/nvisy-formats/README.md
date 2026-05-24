# nvisy-formats

Format-specific loaders and handlers for the Nvisy multimodal redaction
platform.

This crate provides concrete implementations of the trait surface
defined in [`nvisy-codec`] for every supported file format:

- **Text**: TXT, JSON, Markdown, HTML
- **Tabular**: CSV, XLSX
- **Image**: PNG, JPEG, TIFF
- **Audio**: WAV, MP3
- **Rich**: PDF, DOCX

The entry point is [`decode`], which dispatches a [`Content`] to the
appropriate loader by its detected document type and returns a
ready-to-use [`ContentHandle`].

[`nvisy-codec`]: https://docs.rs/nvisy-codec
[`Content`]: nvisy_core::content::Content
[`ContentHandle`]: nvisy_codec::ContentHandle
