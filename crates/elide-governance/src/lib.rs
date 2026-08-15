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
//! [`LabelGroup`]s are named clusters of [`LabelRef`]s a
//! [`Predicate::LabelInGroup`] references by name. Groups live
//! on the policy that declares them (`hipaa_safe_harbor` policy
//! carries a `hipaa_18` group); a rule can reference groups its
//! own policy declared, not another policy's. Membership resolves
//! when the predicate is evaluated, against the group table the
//! policy carries; unknown group names error at validation.
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
//! [`Predicate::LabelInGroup`]: Predicate::LabelInGroup
//! [`Predicated`]: RuleDispatch::Predicated
//! [`Table`]: RuleDispatch::Table

mod policy;
pub mod redaction;

pub use self::policy::{
    LabelEntry, LabelGroup, Labels, PolicyDefinition, PolicyRule, Predicate, RuleDispatch,
    TemplateOrigin,
};
