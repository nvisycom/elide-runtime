//! Entity detection layers and actions.
//!
//! Content-scanning detectors implement [`DetectionLayer`] +
//! [`Detect`] and operate on handler spans.

mod annotation;
mod context;
mod layer;
mod text;
mod tabular;
mod document;

// Traits
pub use context::{DetectionContext, ParallelContext, SequentialContext};
pub use layer::{DetectionLayer, Detect};

// Layers
pub use text::{PatternDetection, PatternDetectionParams};
pub use text::{DictionaryDetection, DictionaryDetectionParams, DictionaryDef};
pub use tabular::{TabularDetection, TabularDetectionParams, ColumnRule};
pub use text::{NerDetection, NerDetectionParams, NerBackend, NerConfig, parse_ner_entities};

// Standalone actions
pub use document::{DetectChecksumAction, DetectChecksumParams};
pub use document::{DetectManualAction, DetectManualParams};

// Types
pub use annotation::{Annotation, AnnotationKind, AnnotationLabel, AnnotationScope};
