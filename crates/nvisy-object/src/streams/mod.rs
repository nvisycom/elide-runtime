//! Streaming read and write adapters for object stores.

pub use nvisy_pipeline::stream::{StreamSource, StreamTarget};

mod read;
mod write;

pub use read::{ObjectReadStream, ObjectReadParams};
pub use write::{ObjectWriteStream, ObjectWriteParams};
