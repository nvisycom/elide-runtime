//! Tabular/spreadsheet format handlers.

#[cfg(feature = "xlsx")]
mod xlsx_handler;
#[cfg(feature = "xlsx")]
mod xlsx_loader;

#[cfg(feature = "xlsx")]
pub use xlsx_handler::XlsxHandler;
#[cfg(feature = "xlsx")]
pub use xlsx_loader::{XlsxLoader, XlsxParams};
