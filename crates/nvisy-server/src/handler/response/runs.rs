//! Run response types.

use nvisy_engine::pipeline::RunEntry;

use crate::handler::request::Page;

/// Response body for `GET /runs`.
pub type RunList = Page<RunEntry>;
