//! File types for the wire and in-memory carriers.
//!
//! [`FileMetadata`] is the persisted descriptor and [`Document`]
//! is the in-memory codec input.
//!
//! A file in the engine is a `(metadata, bytes)` pair: the bytes
//! live in a blob-separated keyspace, the metadata in a small
//! JSON keyspace. The split lets `list_files` enumerate every
//! file for an actor without paying the cost of loading the
//! bytes. Both keyspaces key by `(actor_id, file_id)`.

mod document;
mod metadata;

pub use self::document::Document;
pub use self::metadata::FileMetadata;
