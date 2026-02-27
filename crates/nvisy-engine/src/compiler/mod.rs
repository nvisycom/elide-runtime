//! Pipeline compilation: parsing, graph construction, and execution planning.
//!
//! The compiler takes a JSON pipeline definition, validates it, builds a
//! directed graph, and produces a topologically-sorted execution plan.

pub mod graph;
mod parse;
pub mod plan;
pub mod retry;

pub use graph::{Graph, GraphEdge, GraphNode};
pub use parse::parse_graph;
pub use plan::{build_plan, ExecutionPlan, ResolvedNode};
pub use retry::{BackoffStrategy, RetryPolicy};
