//! Modality-generic dispatch machinery for
//! [`elide_governance::PolicyDefinition`] compilation.
//!
//! Two files:
//!
//! - [`dispatch`]: the modality-generic outer loop that walks
//!   `&[PolicyDefinition]` in precedence order, per-rule dispatches through
//!   a caller-supplied bridge callback. Defines the [`Target`]
//!   enum (rule / fallback / override) that per-modality entries
//!   attach their built operator onto.
//! - [`selector`]: attribution builders (per-rule, per-fallback,
//!   per-override) plus the predicate compiler that maps a wire
//!   [`Predicate`] to an elide selector.
//!
//! Nothing modality-specific lives here. The per-modality entry
//! files ([`super::text`] etc.) pass their [`operator::Op`]
//! builders in as callbacks.
//!
//! [`Predicate`]: elide_governance::Predicate
//! [`operator::Op`]: super::operator

mod dispatch;
mod selector;

pub(in crate::anonymizer) use self::dispatch::{Target, attach_one_override, attach_policies};
