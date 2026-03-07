//! Async stream wrappers for spans and span edits.

mod edit;
mod view;

pub use edit::SpanEditStream;
pub use view::SpanStream;
