//! Detection node configurations: named-entity and pattern-based.
//!
//! Both detection nodes run at **phase 2**, after extraction has converted
//! raw content into text. [`NamedEntityRecognition`] uses language-model
//! inference; [`PatternRecognition`] uses deterministic rules. Their outputs
//! are merged in the subsequent [`Fusion`] node at phase 3.
//!
//! [`Fusion`]: crate::graph::Fusion

mod entity;
mod pattern;

pub use self::entity::NamedEntityRecognition;
pub use self::pattern::PatternRecognition;
