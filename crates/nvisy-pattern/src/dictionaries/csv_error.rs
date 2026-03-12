//! Error type for CSV dictionary parsing.

use nvisy_core::{Error, ErrorKind};

/// Error returned when a CSV dictionary cannot be parsed.
#[derive(Debug, thiserror::Error)]
#[error("failed to parse CSV record in dictionary '{name}': {source}")]
pub struct CsvDictionaryError {
    pub(crate) name: String,
    pub(crate) source: csv::Error,
}

impl From<CsvDictionaryError> for Error {
    fn from(err: CsvDictionaryError) -> Self {
        Error::new(ErrorKind::Validation, err.to_string())
            .with_component("nvisy-pattern::dictionaries")
            .with_source(err)
    }
}
