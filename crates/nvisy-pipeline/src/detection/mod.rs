//! Entity detection actions.
//!
//! Each sub-module exposes a single [`Action`](crate::action::Action)
//! that produces [`Entity`](crate::ontology::entity::Entity) values from
//! document content.

mod checksum;
mod classify;
mod dictionary;
mod manual;
mod ner;
mod regex;
mod tabular;

pub use checksum::{DetectChecksumAction, DetectChecksumParams};
pub use classify::ClassifyAction;
pub use crate::ontology::ClassificationResult;
pub use dictionary::{DetectDictionaryAction, DetectDictionaryParams, DictionaryDef};
pub use manual::{DetectManualAction, DetectManualParams};
pub use ner::{DetectNerAction, DetectNerInput, DetectNerParams, NerBackend, NerConfig, parse_ner_entities};
pub use regex::{DetectRegexAction, DetectRegexParams};
pub use tabular::{ColumnRule, DetectTabularAction, DetectTabularParams};
