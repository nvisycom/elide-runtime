//! Redaction layer: per-modality anonymizer/deanonymizer operators
//! plus the generic [`RedactionRegistry<M>`] that holds
//! deployment-registered custom operators.
//!
//! The toolkit ships:
//!
//! - The [`Anonymizer<M>`] / [`Deanonymizer<M>`] traits operators
//!   implement.
//! - A catalogue of built-in operator structs in [`builtin`]
//!   ([`Replace`], [`Mask`], [`Hash`], [`Redact`],
//!   [`Keep`], [`Encrypt`]). Each is a plain struct; consumers
//!   construct one per rule with whatever params they need.
//! - [`AnonymizerId<M>`] + [`RedactionRegistry<M>`] for the
//!   `Custom` escape hatch: stateful Rust operators (KMS clients,
//!   token vaults, …) that can't be expressed as a config blob.
//!   Built-in operators do **not** round-trip through the registry.
//!
//! Per-modality replacement shapes ([`TextReplacement`],
//! [`ImageReplacement`], [`AudioReplacement`], [`TabularReplacement`])
//! describe what an operator emits at the entity's location.
//!
//! Dispatch (entity → operator) lives one layer up, in the document
//! crate: each rule's `Action::Redact { operator }` carries a closed
//! per-modality enum that either names a built-in (constructed
//! inline) or a `Custom(AnonymizerId<M>)` (looked up in the
//! registry).
//!
//! [`Replace`]: builtin::Replace
//! [`Mask`]: builtin::Mask
//! [`Hash`]: builtin::Hash
//! [`Redact`]: builtin::Redact
//! [`Keep`]: builtin::Keep
//! [`Encrypt`]: builtin::Encrypt

mod anonymizer;
mod deanonymizer;
mod id;
mod leak_profile;
mod registry;

pub mod builtin;

pub use nvisy_core::redaction::{
    AudioReplacement, ImageReplacement, Redactable, TabularReplacement, TextReplacement,
};

pub use self::anonymizer::Anonymizer;
pub use self::deanonymizer::Deanonymizer;
pub use self::id::AnonymizerId;
pub use self::leak_profile::LeakProfile;
pub use self::registry::RedactionRegistry;
