//! Actions a rule can trigger when it matches an entity.
//!
//! Composed via [`PolicyAction`], which the engine dispatches on:
//! [`Redact`] runs the per-modality redaction operator, [`Suppress`]
//! short-circuits redaction for the entity, [`Audit`] records the
//! match for downstream reporting without altering the document.
//!
//! [`PolicyAction`]: super::PolicyAction
//! [`Redact`]: super::PolicyAction::Redact
//! [`Suppress`]: super::PolicyAction::Suppress
//! [`Audit`]: super::PolicyAction::Audit

mod audit;
mod suppress;

pub use self::audit::AuditAction;
pub use self::suppress::SuppressAction;
