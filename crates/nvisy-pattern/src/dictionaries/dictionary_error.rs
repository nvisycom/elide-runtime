//! Error type for dictionary filesystem loading.

use nvisy_core::{Error, ErrorKind};

use super::CsvDictionaryError;

/// Error returned when loading dictionaries from the filesystem.
#[derive(Debug, thiserror::Error)]
pub enum DictionaryLoadError {
    /// The directory could not be read.
    #[error("failed to read dictionary directory '{}': {source}", path.display())]
    ReadDir {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    /// A dictionary file could not be read.
    #[error("failed to read dictionary file '{}': {source}", path.display())]
    ReadFile {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    /// A CSV dictionary file failed to parse.
    #[error("failed to parse CSV dictionary '{}': {source}", path.display())]
    CsvParse {
        path: std::path::PathBuf,
        source: CsvDictionaryError,
    },
}

impl From<DictionaryLoadError> for Error {
    fn from(err: DictionaryLoadError) -> Self {
        let kind = match &err {
            DictionaryLoadError::ReadDir { .. } | DictionaryLoadError::ReadFile { .. } => {
                ErrorKind::Internal
            }
            DictionaryLoadError::CsvParse { .. } => ErrorKind::Validation,
        };
        Error::new(kind, err.to_string())
            .with_component("nvisy-pattern::dictionaries")
            .with_source(err)
    }
}
