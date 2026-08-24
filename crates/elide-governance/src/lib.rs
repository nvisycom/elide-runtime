#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Layers on top of the [elide] toolkit. This crate adds the
//! wire schema for redaction governance: policies, rules,
//! predicates, and operator specs: that the engine walks at
//! apply time.
//!
//! [elide]: https://github.com/nvisycom/elide
//!
//! ## Architecture
//!
//! Authored vocabulary for redaction governance.
//!
//! A request submits `Vec<PolicyDefinition>` in precedence order.
//! Engine walks the policies; for each policy it walks
//! [`PolicyDefinition::rules`] in order and runs the first matching
//! rule's redaction operators. If no rule in a policy matches, the
//! policy's [`PolicyDefinition::fallback`] runs (and the chain
//! halts) if set; otherwise the engine moves to the next policy.
//! If no policy matches and no policy carries a fallback, the
//! entity is skipped.
//!
//! Rules have two shapes ([`PolicyRule`]):
//! - [`Predicated`]: one composable [`Predicate`] gates a single
//!   [`ModalityRedactions`] action.
//! - [`Table`]: a list of per-label [`LabelEntry`] entries: the
//!   compile-time sugar for "route each label to its own operator
//!   under one shared rule identity" (e.g. HIPAA Safe Harbor
//!   fan-out).
//!
//! [`LabelScope`]s are what a policy detects: named, attributed
//! sets of [`LabelRef`]s. Their union is the policy's recognition
//! vocabulary, so a label no scope names is never detected. Rules
//! act within that vocabulary and may target one scope by name via
//! [`Predicate::LabelInScope`]; whatever no rule claims reaches
//! [`fallback`]. Detecting more than the rules act on is the
//! point: scope a regulatory category, write rules for the labels
//! needing special treatment, let the fallback sweep the rest.
//!
//! Scopes are policy-local: a rule can only name a scope its own
//! policy declared, and unknown names error at validation.
//!
//! Identity is UUID-keyed: every [`PolicyDefinition`] and every
//! [`PolicyRule`] carries a stable [`Uuid`](uuid::Uuid). Engine stamps
//! `policy.id` and `rule.id` into the redaction event's
//! [`Attribution`] so reviewers can trace any redaction back to
//! the exact rule that fired.
//!
//! [`Attribution`]: elide_core::entity::audit::Attribution
//! [`LabelRef`]: elide_core::entity::LabelRef
//! [`ModalityRedactions`]: redaction::ModalityRedactions
//! [`Predicate`]: Predicate
//! [`Predicate::LabelInScope`]: Predicate::LabelInScope
//! [`fallback`]: PolicyDefinition::fallback
//! [`Predicated`]: RuleDispatch::Predicated
//! [`Table`]: RuleDispatch::Table

mod catalog;
pub mod modality;
mod policy;
pub mod redaction;

pub use self::catalog::compile_catalog;
pub use self::policy::{
    LabelEntry, LabelScope, PolicyDefinition, PolicyRule, Predicate, RuleDispatch, TemplateOrigin,
};
