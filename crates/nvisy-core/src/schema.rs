//! `JsonSchema` proxies for elide-core types embedded in [`Policy`].
//!
//! Elide-core stays free of `schemars` — schema generation is an
//! HTTP-layer concern that the toolkit doesn't model. These proxies
//! describe each embedded elide type's wire shape so nvisy-core's
//! policy types can derive [`schemars::JsonSchema`] without elide
//! gaining a dep on schemars.
//!
//! Each proxy is used via `#[schemars(with = "ProxyType")]` on the
//! field that holds the elide type. Proxies are never instantiated —
//! they exist only to feed [`schemars::JsonSchema`].
//!
//! [`Policy`]: super::Policy

#![allow(dead_code)]

use schemars::JsonSchema;

/// Schema proxy for [`elide_core::entity::Label`].
#[derive(JsonSchema)]
#[schemars(rename = "Label")]
pub(super) struct LabelSchema {
    /// Stable identifier, e.g. `"email_address"`.
    pub name: String,
    /// Optional human-readable description.
    #[schemars(default)]
    pub description: Option<String>,
    /// Free-form tags policy selectors can target.
    #[schemars(default)]
    pub tags: Vec<String>,
}

/// Schema proxy for [`elide_core::redaction::OperatorId`].
#[derive(JsonSchema)]
#[schemars(rename = "OperatorId")]
pub(super) struct OperatorIdSchema {
    /// Stable operator name (e.g. `"mask"`, `"aes-gcm-encrypt"`).
    pub name: String,
    /// Operator version at the time it was applied.
    pub version: String,
}
