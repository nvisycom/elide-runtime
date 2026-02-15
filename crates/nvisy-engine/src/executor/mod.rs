//! Pipeline execution runtime.
//!
//! Spawns concurrent Tokio tasks for each node in topological order,
//! wires inter-node channels, and collects per-node results.

pub mod context;
pub mod runner;

pub use runner::run_graph;
