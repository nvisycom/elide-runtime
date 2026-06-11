//! Redaction layer: per-modality anonymizer/deanonymizer operators
//! plus the generic [`RedactionRegistry<M>`] that holds
//! deployment-registered custom operators.
//!
//! The toolkit ships:
//!
//! - The [`Anonymizer<M>`] / [`Deanonymizer<M>`] traits operators
//!   implement.
//! - Built-in forward operators in [`anonymizer`] (`Replace`,
//!   `Mask`, `Hash`, `Redact`, `Keep`, `Encrypt`) and reverse
//!   operators in [`deanonymizer`] (`Decrypt`). Each is a plain
//!   struct; consumers construct one per rule with whatever params
//!   they need.
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
//! [`Replace`]: anonymizer::Replace
//! [`Mask`]: anonymizer::Mask
//! [`Hash`]: anonymizer::Hash
//! [`Redact`]: anonymizer::Redact
//! [`Keep`]: anonymizer::Keep
//! [`Encrypt`]: anonymizer::Encrypt

mod id;
mod registry;
mod store;

pub mod anonymizer;
pub mod deanonymizer;

pub use nvisy_core::modality::Modality;
pub use nvisy_core::redaction::{
    Anonymizer, AudioReplacement, Deanonymizer, ImageReplacement, LeakProfile, Memoized, Store,
    TabularReplacement, TextReplacement,
};

pub use self::id::AnonymizerId;
pub use self::registry::RedactionRegistry;
pub use self::store::MemoryStore;
