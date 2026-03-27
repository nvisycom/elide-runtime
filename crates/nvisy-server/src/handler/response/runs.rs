//! Run response types.

use nvisy_engine::pipeline::RunSummary;

use crate::handler::request::Page;

/// Response body for `GET /api/v1/runs`.
pub type RunList = Page<RunSummary>;
