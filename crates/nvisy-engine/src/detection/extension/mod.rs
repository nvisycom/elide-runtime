//! Extension traits used at the recognizer / engine boundary.
//!
//! Currently houses [`RebaseEntities`] for shifting recognizer
//! output from context-local to document-relative byte offsets.

mod rebase;

pub use self::rebase::Rebase;
