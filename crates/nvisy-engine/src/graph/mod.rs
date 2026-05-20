//! Execution extensions for ontology workflow types.
//!
//! The graph data types (nodes, edges, kinds, policies) are defined in
//! [`nvisy_ontology::workflow`]. This module adds:
//!
//! - [`GraphExt`]: petgraph conversion for topological sort / cycle detection.
//! - [`RetryExt`]: automatic retry with configurable backoff.
//! - [`TimeoutExt`]: wall-clock deadline enforcement for pipeline phases.

mod petgraph_ext;
mod retry_ext;
mod timeout_ext;

pub(crate) use self::petgraph_ext::GraphExt;
#[allow(unused_imports)] // wired when operations gain internal retry
pub(crate) use self::retry_ext::RetryExt;
pub(crate) use self::timeout_ext::TimeoutExt;
