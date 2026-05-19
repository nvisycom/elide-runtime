//! Image redaction primitives.

mod instruction;
mod ops;

pub use self::instruction::{ImageOutput, ImageRedaction};
pub(crate) use self::ops::ImageOps;
