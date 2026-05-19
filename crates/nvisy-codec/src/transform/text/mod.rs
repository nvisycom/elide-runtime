//! Text redaction primitives.

mod apply;
mod instruction;

pub(crate) use self::apply::apply_text_redactions;
pub use self::instruction::{TextOutput, TextRedaction};
