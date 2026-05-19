//! Image redaction primitives.

mod instruction;
mod transform;

pub use self::instruction::{ImageOutput, ImageRedaction};
pub use self::transform::ImageTransform;
