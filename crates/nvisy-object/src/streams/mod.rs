//! Streaming traits and object store adapters.

mod source_stream;
mod target_stream;
mod read_object;
mod write_object;

pub use source_stream::StreamSource;
pub use target_stream::StreamTarget;
pub use read_object::{ObjectReadStream, ObjectReadParams};
pub use write_object::{ObjectWriteStream, ObjectWriteParams};
