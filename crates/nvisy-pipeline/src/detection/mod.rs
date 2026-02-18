//! Entity detection layers and actions.
//!
//! Content-scanning detectors implement [`DetectionLayer`] +
//! [`Detect`] and operate on handler spans.

mod annotation;
mod context;
mod layer;
mod pattern;
mod dictionary;
mod tabular;
mod ner;
mod checksum;
mod manual;

// Traits
pub use context::{DetectionContext, ParallelContext, SequentialContext};
pub use layer::{DetectionLayer, Detect};

// Layers
pub use pattern::{PatternDetection, PatternDetectionParams};
pub use dictionary::{DictionaryDetection, DictionaryDetectionParams, DictionaryDef};
pub use tabular::{TabularDetection, TabularDetectionParams, ColumnRule};
pub use ner::{NerDetection, NerDetectionParams, NerBackend, NerConfig, parse_ner_entities};

// Standalone actions
pub use checksum::{DetectChecksumAction, DetectChecksumParams};
pub use manual::{DetectManualAction, DetectManualParams};

// Types
pub use annotation::{Annotation, AnnotationKind, AnnotationLabel, AnnotationScope};
