//! Conflict resolution policy for [`Redactions`].
//!
//! [`Redactions`]: crate::core::Redactions

use thiserror::Error;

/// How [`Redactions`] resolves overlapping insertions within a span.
///
/// [`Redactions`]: crate::core::Redactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ConflictPolicy {
    /// Refuse to insert a redaction that overlaps with an existing one.
    #[default]
    Reject,
    /// Attempt to merge overlapping redactions via
    /// [`Mergeable::try_merge`].
    ///
    /// When `try_merge` returns `None`, insertion fails with
    /// [`InsertError::NotMergeable`].
    ///
    /// [`Mergeable::try_merge`]: crate::core::Mergeable::try_merge
    Merge,
    /// Drop the existing redaction and replace it with the new one.
    Replace,
}

/// Error returned by [`Redactions::try_insert`] when a conflict cannot
/// be resolved under the configured [`ConflictPolicy`].
///
/// [`Redactions::try_insert`]: crate::core::Redactions::try_insert
#[derive(Debug, Error)]
pub enum InsertError {
    /// [`ConflictPolicy::Reject`] is active and the new redaction
    /// overlaps with an existing one.
    #[error("overlapping redaction rejected")]
    OverlapRejected,
    /// [`ConflictPolicy::Merge`] is active but the two redactions
    /// overlap and cannot be merged (e.g. different outputs).
    #[error("overlapping redactions cannot be merged")]
    NotMergeable,
}
