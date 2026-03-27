//! Run response types.

use nvisy_engine::pipeline::RunEntry;

use crate::handler::request::Page;

/// Response body for `GET /api/v1/runs`.
pub type RunList = Page<RunEntry>;
