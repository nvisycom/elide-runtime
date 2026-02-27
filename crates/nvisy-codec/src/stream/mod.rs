//! Async span stream types for the handler pipeline.

mod view_stream;
mod edit_stream;

pub use view_stream::SpanStream;
pub use edit_stream::SpanEditStream;
