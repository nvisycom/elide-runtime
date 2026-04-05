//! Tabular format handlers and loaders (CSV, XLSX).
//!
//! Tabular handlers expose cell-level text spans via [`TextHandler`]
//! and address cells by byte offsets computed from the serialized form.
//!
//! [`TextHandler`]: crate::handler::TextHandler

mod csv_handler;
mod csv_loader;
#[cfg(feature = "xlsx")]
mod xlsx_handler;
#[cfg(feature = "xlsx")]
mod xlsx_loader;

pub use self::csv_handler::{CsvData, CsvHandler};
pub use self::csv_loader::{CsvLoader, CsvParams};
#[cfg(feature = "xlsx")]
pub use self::xlsx_handler::XlsxHandler;
#[cfg(feature = "xlsx")]
pub use self::xlsx_loader::{XlsxLoader, XlsxParams};
