//! Per-request label catalog params.
//!
//! Two distinct sources the engine unions into one
//! [`elide_core::entity::LabelCatalog`] at compile time:
//!
//! - [`builtins`](LabelCatalogParams::builtins) — names of labels
//!   from `elide-core`'s shipped builtin set
//!   ([`LabelCatalog::with_builtins`]). Engine looks each name up
//!   against the full builtin catalog and copies the matching
//!   [`Label`] across; unknown names log a warning and are
//!   skipped (typos don't fail the request).
//! - [`custom`](LabelCatalogParams::custom) — schemas the caller
//!   defined inline, beyond the builtin set. Names that collide
//!   with a builtin replace it (last write wins, matching
//!   [`LabelCatalog::insert`] semantics).
//!
//! Empty default — server-side deployments may pre-populate via
//! the `analyzer` server-default block in the config, but the
//! resolved value goes to the engine empty unless the request or
//! the server-default sets it.
//!
//! [`LabelCatalog::with_builtins`]: elide_core::entity::LabelCatalog::with_builtins
//! [`LabelCatalog::insert`]: elide_core::entity::LabelCatalog::insert
//! [`Label`]: elide_core::entity::Label

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::LabelSchema;

/// Per-request label-catalog selection. Picks builtins by name +
/// adds inline custom schemas.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LabelCatalogParams {
    /// Builtin label names to enable (e.g. `"email_address"`,
    /// `"phone_number"`). Unknown names log a warning and are
    /// skipped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub builtins: Vec<String>,
    /// Custom label schemas defined inline by the caller.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<LabelSchema>,
}
