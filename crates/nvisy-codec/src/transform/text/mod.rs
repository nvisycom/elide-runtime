//! Text redaction primitives.

mod instruction;
mod transform;

pub use self::instruction::{TextOutput, TextRedaction};
pub use self::transform::TextTransform;
