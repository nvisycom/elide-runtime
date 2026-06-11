//! Concrete per-modality format implementations.
//!
//! Each modality module owns the concrete handlers + loaders for
//! every format that produces that modality (TXT/JSON/HTML for
//! text, PNG/JPEG/TIFF for image, …). Rich-document formats (PDF,
//! DOCX) live under [`rich`] because they produce text handles
//! with embedded image children.

#[cfg(feature = "internal_audio")]
pub mod audio;
#[cfg(feature = "internal_image")]
pub mod image;
#[cfg(feature = "internal_rich")]
pub mod rich;
#[cfg(feature = "internal_tabular")]
pub mod tabular;
#[cfg(feature = "internal_text")]
pub mod text;
