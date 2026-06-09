//! Shared constants used across the middleware layer.

use std::time::Duration;

/// Default maximum request body size: 4 MiB.
///
/// Used for security middleware to limit incoming request body sizes
/// and prevent denial-of-service attacks via large payloads.
pub const DEFAULT_MAX_BODY_SIZE: usize = 4 * 1024 * 1024;

/// Maximum file size for uploads: 12 MiB.
///
/// Used in file upload handlers to enforce file size limits
/// before accepting file data into memory.
pub const DEFAULT_MAX_FILE_BODY_SIZE: usize = 12 * 1024 * 1024;

/// Default per-request timeout: 5 minutes.
///
/// Acts as the global ceiling; per-category timeouts below are tighter.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Timeout for read operations (GET): 30 seconds.
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Timeout for write operations (POST, DELETE): 60 seconds.
pub const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(60);

/// Timeout for health checks: 5 seconds.
pub const DEFAULT_HEALTH_TIMEOUT: Duration = Duration::from_secs(5);
