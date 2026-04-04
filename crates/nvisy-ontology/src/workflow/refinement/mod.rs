//! Refinement node configurations: deduplication, redaction, and validation.
//!
//! Refinement nodes form the final processing stages before export.
//! [`Deduplication`] runs at **phase 3** to merge and score entity
//! candidates from all detectors. [`Redaction`] runs at **phase 4** to
//! apply the scored entity list to the document. [`Validation`] runs at
//! **phase 5** to verify that redaction was complete and no values leaked.

mod deduplication;
mod redaction;
mod validation;

pub use self::deduplication::{
    CalibrationMap, Deduplication, DeduplicationStrategy, GroupingCriteria,
};
pub use self::redaction::Redaction;
pub use self::validation::Validation;
