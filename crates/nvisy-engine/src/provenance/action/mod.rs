//! Per-category action payloads for [`AuditEntry`](super::AuditEntry).
//!
//! Each audit entry carries one of these payloads describing what happened
//! during the operation it records.

mod inference;
mod lifecycle;
mod processing;

pub use self::inference::{InferenceAction, InferenceActionBuilder};
pub use self::lifecycle::{LifecycleAction, LifecycleActionBuilder};
pub use self::processing::{ProcessingAction, ProcessingActionBuilder};
