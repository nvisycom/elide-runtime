//! File types for the wire and in-memory carriers.
//!
//! [`FileMetadata`] is the persisted descriptor, [`FileLineage`]
//! carries provenance for engine-produced files, and [`Document`]
//! is the in-memory codec input.
//!
//! A file in the engine is a `(metadata, bytes)` pair: the bytes
//! live in a blob-separated keyspace, the metadata in a small
//! JSON keyspace. The split lets `list_files` enumerate every
//! file for an actor without paying the cost of loading the
//! bytes. Both keyspaces key by `(actor_id, file_id)`.
//!
//! The descriptor mirrors what [`DocumentInput`] carries on a
//! [`StartBatch`]. The same `descriptor_labels` and
//! `descriptor_metadata` gate policies via
//! [`DocumentPredicate`]. When a run references a stored file,
//! the run inherits these gates.
//!
//! [`DocumentInput`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/runs/struct.DocumentInput.html
//! [`StartBatch`]: https://docs.rs/nvisy-engine/latest/nvisy_engine/runs/struct.StartBatch.html
//! [`DocumentPredicate`]: crate::policy::predicate::DocumentPredicate

mod document;
mod lineage;
mod metadata;

pub use self::document::Document;
pub use self::lineage::FileLineage;
pub use self::metadata::FileMetadata;
