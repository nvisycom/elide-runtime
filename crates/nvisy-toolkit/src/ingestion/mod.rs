//! Ingestion: the read-side edge of a toolkit pipeline.
//!
//! Owns the shipped adapters that load per-modality source bytes
//! into memory and satisfy the resolver traits
//! ([`ValueAt`][va], [`SourceAt`][sa]) the detection / deduplication
//! / redaction phases bound on.
//!
//! Today: one type — [`MemoryBuffer<M>`] — covers the in-memory
//! case. Future I/O adapters (HTTP fetch, S3 reader, mmap) plug in
//! as siblings without changing the consumer-side trait surface.
//!
//! [va]: nvisy_core::extraction::ValueAt
//! [sa]: nvisy_core::extraction::SourceAt

mod memory;

pub use self::memory::MemoryBuffer;
