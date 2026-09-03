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
/// request can point the reviewer at the edit to fix. Who made that
/// edit is not here: the caller still holds the set it submitted,
/// and the entity plus the operation names identify the pair.
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
    },
    /// An add names a part the report does not hold *for this
    /// modality* — an unknown path, or a real one holding another
    /// medium.
    ///
    /// Rejected rather than skipped for the same reason as an
    /// unknown target: elide's `include_part` returns `false` for
    /// both, equally silently, so the addition would vanish and the
    /// reviewer would be told it landed.
    UnknownPart {
        /// The part path the edit names, top-level document first.
        part: Vec<String>,
        /// The modality the add carries, which is what the part was
        /// searched for.
        modality: &'static str,
    },
    /// An add leaves its part unset against a report describing
    /// several documents, so "the one I sent" names nothing.
    ///
    /// An unset part is shorthand for the sole document, which only
    /// resolves when the report holds exactly one. With several,
    /// there is nothing to resolve it to and the addition would
    /// silently go nowhere.
    AmbiguousPart {
        /// How many documents the report describes. Always more
        /// than one: an empty report is [`EmptyReport`] instead,
        /// and exactly one is the case this variant excludes.
        ///
        /// [`EmptyReport`]: Self::EmptyReport
        documents: usize,
    },
    /// An edit is applied to a report describing no document at
    /// all.
    ///
    /// Distinct from [`AmbiguousPart`]: naming a part cannot help,
    /// because the report holds none. A report is empty only when
    /// analysis never ran, or ran against a document the
    /// orchestrator has no pipeline for, so the fix is upstream of
    /// the edit.
    ///
    /// [`AmbiguousPart`]: Self::AmbiguousPart
    EmptyReport,
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
    /// The entity this error is about, when it names one.
    #[must_use]
    pub fn entity_id(&self) -> Option<Uuid> {
        match self {
            Self::Contradiction { entity_id, .. } | Self::UnknownTarget { entity_id, .. } => {
                Some(*entity_id)
            }
            // An add names no entity: the engine mints the id when
            // the edit lands.
            Self::UnknownPart { .. } | Self::AmbiguousPart { .. } | Self::EmptyReport => None,
        }
    }

    /// A contradiction between two edits on `M`.
    pub(crate) fn contradiction<M: Modality>(
        entity_id: Uuid,
        earlier: &'static str,
        later: &'static str,
    ) -> Self {
        Self::Contradiction {
            entity_id,
            modality: M::NAME,
            earlier,
            later,
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
            } => write!(
                f,
                "{modality} entity `{entity_id}` carries contradictory edits: \
                 `{earlier}` and `{later}` answer the same question differently. \
                 Send one.",
            ),
            Self::UnknownPart { part, modality } => write!(
                f,
                "no `{modality}` part `{}` in this report: the add names \
                 a part the document does not carry, or one holding a \
                 different medium.",
                part.join(" / "),
            ),
            Self::AmbiguousPart { documents } => write!(
                f,
                "the add leaves its part unset, which means the sole \
                 document, but this report describes {documents} of them. \
                 Name the part the addition belongs to.",
            ),
            Self::EmptyReport => f.write_str(
                "this report describes no document, so there is nothing \
                 to edit: analyze must run first.",
            ),
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
