//! Per-policy label catalog: the vocabulary the policy's rules
//! and predicates operate over.
//!
//! Two distinct sources the engine unions into one
//! `elide_core::entity::LabelCatalog` at request-compile time:
//!
//! - [`builtins`]: names of labels from `elide-core`'s shipped
//!   builtin set (`LabelCatalog::with_builtins`). Engine looks
//!   each name up against the full builtin catalog and copies
//!   the matching [`Label`] across; unknown names log a warning
//!   and are skipped (typos don't fail the request).
//! - [`custom`]: schemas the caller defined inline, beyond the
//!   builtin set. Names that collide with a builtin replace it
//!   (last write wins, matching `LabelCatalog::insert` semantics).
//!
//! Empty default. Every submitted [`PolicyDefinition`]'s labels
//! union at request time to form the analyzer's per-run catalog.
//!
//! [`builtins`]: Labels::builtins
//! [`custom`]: Labels::custom
//! [`PolicyDefinition`]: super::PolicyDefinition

use elide_core::entity::{Label, LabelRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Per-policy label-catalog selection.
///
/// Picks builtins by name + adds inline custom schemas.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Labels {
    /// Builtin label names to enable.
    ///
    /// E.g. `"email_address"`, `"phone_number"`. Unknown names
    /// log a warning and are skipped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub builtins: Vec<LabelRef>,
    /// Custom labels defined inline by the caller.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<Label>,
}

impl Labels {
    /// `true` when neither source contributes any label.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.builtins.is_empty() && self.custom.is_empty()
    }
}
