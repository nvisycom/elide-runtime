//! Binary document loaders (PDF, DOCX).

#[cfg(feature = "pdf")]
pub mod pdf;

#[cfg(feature = "docx")]
pub mod docx;
