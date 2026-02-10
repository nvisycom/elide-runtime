use std::fmt;

/// Classification of error kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Validation,
    Connection,
    Timeout,
    Cancellation,
    Policy,
    Runtime,
    Python,
    Other,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation => write!(f, "Validation"),
            Self::Connection => write!(f, "Connection"),
            Self::Timeout => write!(f, "Timeout"),
            Self::Cancellation => write!(f, "Cancelled"),
            Self::Policy => write!(f, "Policy"),
            Self::Runtime => write!(f, "Runtime"),
            Self::Python => write!(f, "Python"),
            Self::Other => write!(f, "Other"),
        }
    }
}

/// Unified error type for the Nvisy platform.
#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    pub source_component: Option<String>,
    pub retryable: bool,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source_component: None,
            retryable: false,
            source: None,
        }
    }

    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        self.source_component = Some(component.into());
        self
    }

    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    pub fn validation(message: impl Into<String>, source: impl Into<String>) -> Self {
        Self::new(ErrorKind::Validation, message).with_component(source)
    }

    pub fn connection(
        message: impl Into<String>,
        source: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self::new(ErrorKind::Connection, message)
            .with_component(source)
            .with_retryable(retryable)
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Timeout, message).with_retryable(true)
    }

    pub fn cancellation(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Cancellation, message)
    }

    pub fn policy(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Policy, message)
    }

    pub fn runtime(
        message: impl Into<String>,
        source: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self::new(ErrorKind::Runtime, message)
            .with_component(source)
            .with_retryable(retryable)
    }

    pub fn python(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Python, message)
    }

    /// Whether this error is retryable.
    pub fn is_retryable(&self) -> bool {
        self.retryable
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        // anyhow::Error doesn't implement std::error::Error, so we capture the
        // full chain as text instead of storing it as a boxed source.
        Self::new(ErrorKind::Other, format!("{err:#}"))
    }
}

/// Convenience type alias for results using the Nvisy error type.
pub type Result<T> = std::result::Result<T, Error>;

// Keep backward compatibility: NvisyError is an alias for Error.
pub type NvisyError = Error;
