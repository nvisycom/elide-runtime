//! Execution extensions for ontology workflow types.
//!
//! The graph data types (nodes, edges, kinds, policies) are defined in
//! [`nvisy_ontology::workflow`]. This module adds:
//!
//! - [`GraphExt`]: petgraph conversion for topological sort / cycle detection.
//! - [`RetryExt`]: automatic retry with configurable backoff.
//! - [`TimeoutExt`]: wall-clock deadline enforcement for pipeline phases.

mod petgraph;
mod retry;
mod timeout;

pub(crate) use self::petgraph::GraphExt;
#[allow(unused_imports)] // wired when operations gain internal retry
pub(crate) use self::retry::RetryExt;
pub(crate) use self::timeout::TimeoutExt;
