/// Unified error type for the Nvisy platform.
#[derive(Debug, thiserror::Error)]
pub enum NvisyError {
    #[error("Validation: {message}")]
    Validation {
        message: String,
        source_component: String,
    },

    #[error("Connection: {message}")]
    Connection {
        message: String,
        source_component: String,
        retryable: bool,
    },

    #[error("Timeout: {message}")]
    Timeout { message: String },

    #[error("Cancelled: {message}")]
    Cancellation { message: String },

    #[error("Policy: {message}")]
    Policy { message: String },

    #[error("Runtime: {message}")]
    Runtime {
        message: String,
        source_component: String,
        retryable: bool,
    },

    #[error("Python: {message}")]
    Python {
        message: String,
        traceback: Option<String>,
    },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl NvisyError {
    pub fn validation(message: impl Into<String>, source: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
            source_component: source.into(),
        }
    }

    pub fn connection(
        message: impl Into<String>,
        source: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self::Connection {
            message: message.into(),
            source_component: source.into(),
            retryable,
        }
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::Timeout {
            message: message.into(),
        }
    }

    pub fn cancellation(message: impl Into<String>) -> Self {
        Self::Cancellation {
            message: message.into(),
        }
    }

    pub fn policy(message: impl Into<String>) -> Self {
        Self::Policy {
            message: message.into(),
        }
    }

    pub fn runtime(
        message: impl Into<String>,
        source: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self::Runtime {
            message: message.into(),
            source_component: source.into(),
            retryable,
        }
    }

    pub fn python(message: impl Into<String>, traceback: Option<String>) -> Self {
        Self::Python {
            message: message.into(),
            traceback,
        }
    }

    /// Whether this error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Connection { retryable, .. } => *retryable,
            Self::Runtime { retryable, .. } => *retryable,
            Self::Timeout { .. } => true,
            _ => false,
        }
    }
}
