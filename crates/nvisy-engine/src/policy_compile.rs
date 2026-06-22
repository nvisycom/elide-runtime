//! Compile a [`nvisy_core::policy::Policy`] into an
//! [`elide::Anonymizer`] at request time.
//!
//! The policy spec is serialisable and modality-agnostic; elide's
//! [`Anonymizer<M>`] is a runtime, modality-typed value that drives
//! actual redaction. This module bridges the two: it walks every
//! enabled rule in precedence order, builds the matching elide
//! operator from the spec, wraps it in a decorator that stamps the
//! audit with [`PolicyDecisionRef`], and attaches it to the
//! anonymizer with a predicate built from the rule's selector and
//! conditions.
//!
//! Today the module is a placeholder; the compile surface lands in
//! the follow-up patch.
//!
//! [`Anonymizer<M>`]: elide::Anonymizer
//! [`PolicyDecisionRef`]: nvisy_core::policy::PolicyDecisionRef
