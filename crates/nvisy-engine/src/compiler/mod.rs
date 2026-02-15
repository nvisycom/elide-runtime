//! Pipeline compilation: parsing, graph construction, and execution planning.
//!
//! The compiler takes a JSON pipeline definition, validates it, builds a
//! directed graph, and produces a topologically-sorted execution plan.

pub mod graph;
pub mod parse;
pub mod plan;

pub use parse::parse_graph;
pub use plan::{build_plan, ExecutionPlan, ResolvedNode};
