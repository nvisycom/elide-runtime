//! [`EntityLabel`] — open vocabulary tag for detected entities.
//!
//! Any recognizer can mint a label by name, ship it through the
//! pipeline, and have the audit reference it verbatim. The workspace
//! ships a catalog of built-in labels in [`super::builtins`];
//! recognizers and policy authors are free to invent new ones
//! (`acme-internal-id`, `medical-record-no`) without touching
//! workspace code.
//!
//! ## Identity
//!
//! Labels are identified by [`name`]; two labels with the same name
//! are considered the same entity kind regardless of differences in
//! [`description`] or [`tags`]. Selectors match by name.
//!
//! ## Tags
//!
//! [`tags`] is a free-form list of short identifiers that policy
//! selectors can match against. Built-in labels carry category
//! tags (`personal_identity`, `contact_info`, `financial`, etc.)
//! plus cross-cutting tags (`pii`, `phi`, `pci`). Custom labels
//! can ship with zero tags; selectors targeting tags only match
//! labels that carry them.
//!
//! [`name`]: EntityLabel::name
//! [`description`]: EntityLabel::description
//! [`tags`]: EntityLabel::tags

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::EntityLabelRef;

/// Open-vocabulary entity label: identity, optional description,
/// and zero or more tags.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntityLabel {
    /// Canonical name of the label (e.g. `"person_name"`,
    /// `"acme_internal_id"`). Selectors match by this value.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Optional human-readable description of what the label
    /// represents. Surfaced in audits and policy author tooling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub description: Option<HipStr<'static>>,
    /// Free-form tags grouping this label with related ones.
    /// Built-in labels carry category tags
    /// (`personal_identity`, `financial`, …) plus cross-cutting
    /// tags where applicable (`pii`, `phi`, `pci`). Empty for
    /// untagged custom labels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<String>")]
    pub tags: Vec<HipStr<'static>>,
}

impl EntityLabel {
    /// Construct a label from a runtime name. Description and tags
    /// default to empty; use [`Self::with_description`] and
    /// [`Self::with_tags`] to add them.
    pub fn new(name: impl Into<HipStr<'static>>) -> Self {
        Self {
            name: name.into(),
            description: None,
            tags: Vec::new(),
        }
    }

    /// Construct a label entirely from `&'static str` literals.
    /// Used by the built-in catalog in [`super::builtins`] so the
    /// strings live in static storage and runtime construction is
    /// just one `Vec::from` per built-in.
    pub fn from_static(
        name: &'static str,
        description: Option<&'static str>,
        tags: &'static [&'static str],
    ) -> Self {
        Self {
            name: HipStr::from_static(name),
            description: description.map(HipStr::from_static),
            tags: tags.iter().copied().map(HipStr::from_static).collect(),
        }
    }

    /// Attach a description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<HipStr<'static>>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Attach tags. Replaces any previously set tags.
    #[must_use]
    pub fn with_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<HipStr<'static>>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Returns `true` when this label carries `tag` in its tag
    /// list. Comparison is byte-for-byte.
    #[must_use]
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Construct a name-only [`EntityLabelRef`][super::EntityLabelRef]
    /// handle to this label. Clones the underlying [`HipStr`]
    /// (a refcount bump for `from_static` labels — no allocation).
    #[must_use]
    pub fn label_ref(&self) -> EntityLabelRef {
        EntityLabelRef::from(self.name.clone())
    }
}

impl AsRef<str> for EntityLabel {
    fn as_ref(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Display for EntityLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.name, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_static_round_trips() {
        let l = EntityLabel::from_static(
            "email_address",
            Some("Email address."),
            &["contact_info", "pii"],
        );
        assert_eq!(l.name, "email_address");
        assert_eq!(l.description.as_deref(), Some("Email address."));
        assert!(l.has_tag("contact_info"));
        assert!(l.has_tag("pii"));
        assert!(!l.has_tag("financial"));
    }

    #[test]
    fn builder_setters_chain() {
        let l = EntityLabel::new("acme_internal_id")
            .with_description("ACME corp internal record id")
            .with_tags(["custom", "acme"]);
        assert_eq!(l.name, "acme_internal_id");
        assert_eq!(
            l.description.as_deref(),
            Some("ACME corp internal record id"),
        );
        assert!(l.has_tag("acme"));
    }

    #[test]
    fn equality_ignores_metadata() {
        // NOTE: deliberately *not* the behaviour today — `derive(PartialEq)`
        // makes equality structural. If selectors need name-only equality
        // they should compare `.name` explicitly. This test documents the
        // current contract so a future change is intentional.
        let a = EntityLabel::new("person_name").with_tags(["pii"]);
        let b = EntityLabel::new("person_name");
        assert_ne!(a, b);
    }
}
