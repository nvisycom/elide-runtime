//! Cross-modal NER backend trait, configuration, detection layers, and
//! result parsing.

mod backend;
mod bridge;
mod parse;
pub mod text;
pub mod image;

pub use backend::{NerBackend, NerConfig};
pub use parse::{parse_image_ner_entity, parse_ner_entities};
pub use text::{NerDetection, NerDetectionParams};
pub use image::ImageNerDetection;
