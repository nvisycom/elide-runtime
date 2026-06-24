//! Run orchestrator: persists analyze + apply lifecycles over
//! the fjall registry, exposes start/get/list/override/apply.
//!
//! Today the lifecycle persistence + reviewer-override surface
//! lands; the analyzer fan-out (codec decode → recognizer →
//! per-doc body update) and the apply-time anonymizer pass are
//! flagged TODO in [`orchestrate`] and follow in the next slice.
//!
//! Symmetric with [`crate::policies`] / [`crate::contexts`]: all
//! engine state lives in fjall keyspaces on the shared
//! [`RegistryHandle`]; per-call functions read/write through it.
//!
//! [`RegistryHandle`]: crate::registry::RegistryHandle

mod filter;
mod input;
mod orchestrate;
mod persist;
mod pipeline;
mod state;

pub use self::input::{DocumentInput, StartBatch};
pub use self::orchestrate::{apply, override_entity, start};
pub use self::persist::{get_artifact, get_doc, get_header};
pub use self::state::{
    DocBody, EntityRecord, FailureReason, ModalityKind, ResourceRef, Run, RunDocState,
    RunDocument, RunState,
};
