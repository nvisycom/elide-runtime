//! Tabular/spreadsheet file loaders (XLSX, Parquet).

#[cfg(feature = "xlsx")]
pub mod xlsx;

#[cfg(feature = "parquet")]
pub mod parquet;
