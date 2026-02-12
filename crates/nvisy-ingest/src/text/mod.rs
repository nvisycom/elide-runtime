//! Text-based file loaders (CSV, JSON, plaintext, HTML).

pub mod csv;
pub mod json;
pub mod plaintext;

#[cfg(feature = "html")]
pub mod html;
