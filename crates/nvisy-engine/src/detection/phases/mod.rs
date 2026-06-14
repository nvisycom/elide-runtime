//! Detection-side per-document phase orchestrators.
//!
//! Each phase is a document-walking driver around its toolkit-side
//! subsystem (extractor registry, recognizer registry, dedup
//! layers). Phases own the per-document state, the sequencing
//! through `Document<M>` blocks / nodes, and the conversion
//! between toolkit-shaped library calls and the typed document
//! audit.
//!
//! - [`ExtractionPhase`] runs phase 1: pulls chunks through the
//!   codec, runs the matching toolkit-side extractor (OCR / STT
//!   for image / audio), and writes per-modality `Block`s onto
//!   each `Document<M>`.
//! - [`DetectionPhase`] runs phase 2: walks each `Document<M>`'s
//!   blocks, dispatches text + image content through the
//!   per-request `RecognizerRegistry`, and appends detected
//!   entities to the document's audit.
//! - [`DeduplicationPhase`] runs phase 3: merges co-referent
//!   detections across recognizers using the toolkit's
//!   [`LayerPipeline`].
//!
//! [`LayerPipeline`]: nvisy_toolkit::deduplication::LayerPipeline

pub mod deduplication;
pub mod detection;
pub mod extraction;

pub use self::deduplication::DeduplicationPhase;
pub use self::detection::DetectionPhase;
pub use self::extraction::ExtractionPhase;
