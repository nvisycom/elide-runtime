//! Text-based format handlers.

mod txt_handler;
mod txt_loader;
mod csv_handler;
mod csv_loader;
mod json_handler;
mod json_loader;
#[cfg(feature = "html")]
mod html;

pub use txt_handler::{TxtData, TxtHandler, TxtSpan};
pub use txt_loader::{TxtLoader, TxtParams};
pub use csv_handler::{CsvData, CsvHandler, CsvSpan};
pub use csv_loader::{CsvLoader, CsvParams};
pub use json_handler::{JsonData, JsonHandler, JsonIndent, JsonPath};
pub use json_loader::{JsonLoader, JsonParams};
#[cfg(feature = "html")]
pub use html::HtmlHandler;
