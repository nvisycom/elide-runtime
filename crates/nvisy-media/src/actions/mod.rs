//! Pipeline actions for applying redactions to media (images, tabular data, PDFs).

/// Applies image redactions (blur, block) to image artifacts.
pub mod apply_image_redaction;
/// Applies redactions to tabular data cells.
pub mod apply_tabular_redaction;
/// Reassembles redacted content into PDF files.
pub mod apply_pdf_redaction;
/// Placeholder for audio redaction.
pub mod apply_audio_redaction;
