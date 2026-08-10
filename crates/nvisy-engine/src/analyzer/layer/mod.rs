//! Post-recognition analyzer layers.
//!
//! Runs after every recognizer + enricher has attached. Today
//! this is just [`dedup`] (reconcile → filter); the module is a
//! placeholder for future post-processing layers (e.g. entity
//! resolution, cross-modality dedup).

mod dedup;

pub(super) use self::dedup::attach as attach_dedup;
