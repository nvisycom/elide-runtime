//! Azure Document Intelligence backend.
//!
//! Uses the async two-step flow: POST to start analysis, then poll GET
//! until results are available. Parses word-level quadrilateral polygons.

mod backend;
mod params;

pub use self::backend::AzureDocaiBackend;
pub use self::params::AzureDocaiParams;
