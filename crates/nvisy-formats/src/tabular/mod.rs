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
pub use self::csv_handler::{CsvData, CsvHandler};
#[cfg(feature = "csv")]
pub use self::csv_loader::{CsvLoader, CsvParams};
#[cfg(feature = "xlsx")]
pub use self::xlsx_handler::XlsxHandler;
#[cfg(feature = "xlsx")]
pub use self::xlsx_loader::{XlsxLoader, XlsxParams};
