//! Cross-modal NER backend trait, configuration, and result parsing.

mod backend;
mod bridge;
mod parse;

pub use backend::{NerBackend, NerConfig};
pub use parse::{parse_image_ner_entity, parse_ner_entities};
