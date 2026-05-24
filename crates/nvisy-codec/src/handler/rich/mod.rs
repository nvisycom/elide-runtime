//! Rich-document handler trait + boxed wrapper.
//!
//! Rich documents (PDF, DOCX, …) expose both text and image
//! content; [`RichHandler`] combines the [`TextHandler`] and
//! [`ImageHandler`] capabilities into a single trait object so the
//! wrapper can route operations to either modality.
//!
//! Concrete per-format implementations live in `nvisy-formats`.
//!
//! [`TextHandler`]: crate::handler::TextHandler
//! [`ImageHandler`]: crate::handler::ImageHandler

mod boxed;

pub use self::boxed::{BoxedRichHandler, RichHandler};
