//! Error type for pattern filesystem loading.

use nvisy_core::{Error, ErrorKind};

use super::json_pattern::JsonPatternError;

/// Error returned when loading patterns from the filesystem.
#[derive(Debug, thiserror::Error)]
pub enum PatternLoadError {
    /// The directory could not be read.
    #[error("failed to read pattern directory '{}': {source}", path.display())]
    ReadDir {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    /// A pattern file could not be read.
    #[error("failed to read pattern file '{}': {source}", path.display())]
    ReadFile {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    /// A pattern file failed to parse.
    #[error("failed to parse pattern '{}': {source}", path.display())]
    Parse {
        path: std::path::PathBuf,
        source: JsonPatternError,
    },
}

impl From<PatternLoadError> for Error {
    fn from(err: PatternLoadError) -> Self {
        let kind = match &err {
            PatternLoadError::ReadDir { .. } | PatternLoadError::ReadFile { .. } => {
                ErrorKind::Internal
            }
            PatternLoadError::Parse { .. } => ErrorKind::Validation,
        };
        Error::new(kind, err.to_string())
            .with_component("nvisy-pattern::patterns")
            .with_source(err)
    }
}
