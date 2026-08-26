//! [`EditError`]: why an edit set cannot be applied to a report.
//!
//! Structured rather than a formatted string, because the caller
//! that hits one is usually answering an HTTP request: it needs the
//! entity to point at and the reason to classify, not prose to
//! re-parse.

use core::fmt;

use elide::modality::Modality;
use elide::{Error, ErrorKind};
use uuid::Uuid;

/// Why an [`EditSet`](super::EditSet) cannot be applied.
///
/// Both variants name the entity at fault, so a caller answering a
/// request can point the reviewer at the edit to fix.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditError {
    /// Two edits answer the same question about one entity
    /// differently — two labels for one retag, say. Neither can be
    /// preferred without discarding a decision a reviewer made, so
    /// the set is rejected instead.
    Contradiction {
        /// The entity both edits name.
        entity_id: Uuid,
        /// The modality whose bucket holds them.
        modality: &'static str,
        /// The operation seen first.
        earlier: &'static str,
        /// The operation that contradicts it.
        later: &'static str,
        /// Who made each, when the payload named them: a reviewer
        /// reconciling this wants to know who disagreed.
        actors: Option<(String, String)>,
    },
    /// An edit names an entity the report does not hold — a stale
    /// id, or one filed under the wrong modality.
    ///
    /// Rejected rather than skipped: applying is silent about a
    /// target it cannot find, so a reviewer would be told their
    /// decision took effect when the document says otherwise.
    UnknownTarget {
        /// The entity the edit names.
        entity_id: Uuid,
        /// The modality bucket it was filed under, which is where
        /// the report was searched.
        modality: &'static str,
    },
}

impl EditError {
    /// The entity this error is about.
    #[must_use]
    pub const fn entity_id(&self) -> Uuid {
        match self {
            Self::Contradiction { entity_id, .. } | Self::UnknownTarget { entity_id, .. } => {
                *entity_id
            }
        }
    }

    /// A contradiction between two edits on `M`.
    pub(crate) fn contradiction<M: Modality>(
        entity_id: Uuid,
        earlier: &'static str,
        later: &'static str,
        actors: Option<(String, String)>,
    ) -> Self {
        Self::Contradiction {
            entity_id,
            modality: M::NAME,
            earlier,
            later,
            actors,
        }
    }

    /// An edit on `M` naming an entity the report does not hold.
    pub(crate) fn unknown_target<M: Modality>(entity_id: Uuid) -> Self {
        Self::UnknownTarget {
            entity_id,
            modality: M::NAME,
        }
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contradiction {
                entity_id,
                modality,
                earlier,
                later,
                actors,
            } => {
                write!(
                    f,
                    "{modality} entity `{entity_id}` carries contradictory edits: \
                     `{earlier}` and `{later}` answer the same question \
                     differently. Send one.",
                )?;
                match actors {
                    Some((a, b)) => write!(f, " (from `{a}` and `{b}`)"),
                    None => Ok(()),
                }
            }
            Self::UnknownTarget {
                entity_id,
                modality,
            } => write!(
                f,
                "no {modality} entity `{entity_id}` in this report: the id is \
                 stale, or the edit is filed under the wrong modality.",
            ),
        }
    }
}

impl core::error::Error for EditError {}

impl From<EditError> for Error {
    /// So a caller with no interest in the distinction can use `?`
    /// against elide's own error type.
    fn from(error: EditError) -> Self {
        Self::new(ErrorKind::Configuration, error.to_string())
    }
}
