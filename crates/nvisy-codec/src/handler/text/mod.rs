//! Text-based format handlers.

pub mod txt_handler;
pub mod txt_loader;
pub mod csv_handler;
pub mod csv_loader;
pub mod json_handler;
pub mod json_loader;
#[cfg(feature = "html")]
pub mod html;
