//! Ingestion: the read-side edge of a toolkit pipeline.
//!
//! Owns the shipped adapters that load per-modality source bytes
//! into memory and satisfy the resolver traits
//! ([`TextAt`], [`DataAt`]) the detection / deduplication
//! / redaction phases bound on.
//!
//! Today: one type — [`MemoryBuffer<M>`] — covers the in-memory
//! case. Future I/O adapters (HTTP fetch, S3 reader, mmap) plug in
//! as siblings without changing the consumer-side trait surface.
//!
//! [`TextAt`]: nvisy_core::extraction::TextAt
//! [`DataAt`]: nvisy_core::extraction::DataAt

mod memory;

pub use self::memory::MemoryBuffer;
