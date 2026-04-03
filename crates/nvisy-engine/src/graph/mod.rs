//! Execution extensions for ontology graph types.
//!
//! The graph data types (nodes, edges, kinds, policies) are defined in
//! [`nvisy_ontology::workflow`]. This module adds async execution
//! behavior via extension traits.

#[allow(dead_code)]
mod concurrency;
mod retry;
#[allow(dead_code)]
mod timeout;

pub(crate) use self::retry::RetryExt;
