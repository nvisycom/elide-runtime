//! Tabular-format implementations: CSV, XLSX.

#[cfg(feature = "csv")]
mod csv_handler;
#[cfg(feature = "csv")]
mod csv_loader;
#[cfg(feature = "xlsx")]
mod xlsx_handler;
#[cfg(feature = "xlsx")]
mod xlsx_loader;

#[cfg(feature = "csv")]
pub use self::csv_handler::{CsvData, CsvHandler, format as csv_format};
#[cfg(feature = "csv")]
pub use self::csv_loader::CsvLoader;
#[cfg(feature = "xlsx")]
pub use self::xlsx_handler::{XlsxHandler, format as xlsx_format};
#[cfg(feature = "xlsx")]
pub use self::xlsx_loader::XlsxLoader;
