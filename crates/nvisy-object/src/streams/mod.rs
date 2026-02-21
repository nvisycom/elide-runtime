//! Streaming read and write adapters for object stores.

mod read;
mod write;

pub use read::{ObjectReadStream, ObjectReadParams};
pub use write::{ObjectWriteStream, ObjectWriteParams};
