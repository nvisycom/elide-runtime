//! Per-document phase orchestrators.
//!
//! Each phase is a document-walking driver around its toolkit-side
//! subsystem (recognizer registry, dedup layers, leak checks,
//! redaction strategies). Phases own the per-document state, the
//! sequencing through `Document<M>` blocks / nodes, and the
//! conversion between toolkit-shaped library calls and the typed
//! document audit.
//!
//! Toolkit subsystems stay free of [`Document<M>`] knowledge so they
//! can be exercised standalone (tests, custom drivers). Phases are
//! the only place document- and toolkit-shape types meet.
//!
//! [`Document<M>`]: crate::document::Document

pub mod deduplication;
pub mod detection;
pub mod extraction;
pub mod ingestion;
pub mod redaction;
pub mod validation;
