//! Caller-supplied region annotations passed to the analyzer.
//!
//! Two directions, two types. An [`Inclusion`] adds a candidate
//! region ("there may be an entity here"); recognizers that
//! adjudicate it (typically LLM-based) fold it into detection.
//! An [`Exclusion`] removes ("flag nothing here"); the analyzer
//! drops any entity overlapping it, regardless of which
//! recognizer found it.
//!
//! Both are modality-typed: they attach to the analyzer of their
//! own modality and carry a modality-native `M::Location`. Wire
//! types on [`plan`] bag them per modality via
//! [`plan::AnyAnnotations`].
//!
//! [`plan`]: crate::plan
//! [`plan::AnyAnnotations`]: crate::plan::AnyAnnotations

pub use elide_core::recognition::annotation::{Annotations, Exclusion, Inclusion};
