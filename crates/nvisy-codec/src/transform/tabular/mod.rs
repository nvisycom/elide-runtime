//! Tabular redaction primitives.

mod apply;
mod instruction;

pub(crate) use self::apply::apply_tabular_redactions;
pub use self::instruction::TabularRedaction;
