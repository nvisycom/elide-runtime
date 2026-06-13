//! [`EntityLabelRef`] — name-only handle to an [`EntityLabel`].
//!
//! Per-entity hot paths (`Entity::label`, audit refs, selector
//! matching) carry only the label's identifying name, not its full
//! catalog metadata. [`EntityLabelRef`] wraps a [`HipStr<'static>`]
//! so the surface is a single newtype rather than a bare string,
//! giving us a typed receiver for ergonomics like
//! `entity.label.matches("payment_card")`.
//!
//! Catalog-side metadata (description, tags) lives on
//! [`EntityLabel`] and is dereferenced through
//! [`EntityLabelCatalog::lookup`] when a consumer needs it.
//!
//! [`EntityLabel`]: super::EntityLabel
//! [`EntityLabelCatalog::lookup`]: super::EntityLabelCatalog::lookup

use std::fmt;

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Name-only handle to an [`EntityLabel`]. Cheap-clone wrapper
/// around [`HipStr<'static>`].
///
/// Carried on every [`Entity`] in place of the full catalog
/// metadata. Two refs are equal when their names are equal
/// byte-for-byte.
///
/// [`EntityLabel`]: super::EntityLabel
/// [`Entity`]: crate::entity::Entity
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[schemars(with = "String")]
pub struct EntityLabelRef(HipStr<'static>);

impl EntityLabelRef {
    /// Wrap a name.
    pub fn new(name: impl Into<HipStr<'static>>) -> Self {
        Self(name.into())
    }

    /// Wrap a `&'static str` without allocating.
    #[must_use]
    pub const fn from_static(name: &'static str) -> Self {
        Self(HipStr::from_static(name))
    }

    /// Borrow the underlying name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Borrow the inner [`HipStr`].
    #[must_use]
    pub fn as_hipstr(&self) -> &HipStr<'static> {
        &self.0
    }

    /// Consume the ref and return the inner [`HipStr`].
    #[must_use]
    pub fn into_hipstr(self) -> HipStr<'static> {
        self.0
    }

    /// `true` when this ref names `name` byte-for-byte.
    #[must_use]
    pub fn matches(&self, name: &str) -> bool {
        self.0 == name
    }
}

impl AsRef<str> for EntityLabelRef {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for EntityLabelRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<HipStr<'static>> for EntityLabelRef {
    fn from(value: HipStr<'static>) -> Self {
        Self(value)
    }
}

impl From<EntityLabelRef> for HipStr<'static> {
    fn from(value: EntityLabelRef) -> Self {
        value.0
    }
}

impl From<&'static str> for EntityLabelRef {
    fn from(value: &'static str) -> Self {
        Self::from_static(value)
    }
}

impl From<String> for EntityLabelRef {
    fn from(value: String) -> Self {
        Self(HipStr::from(value))
    }
}

impl PartialEq<str> for EntityLabelRef {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for EntityLabelRef {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_static_no_alloc() {
        let r = EntityLabelRef::from_static("payment_card");
        assert_eq!(r.as_str(), "payment_card");
        assert!(r.matches("payment_card"));
        assert!(!r.matches("person_name"));
    }

    #[test]
    fn equality_with_str() {
        let r = EntityLabelRef::from_static("email_address");
        assert_eq!(r, "email_address");
    }

    #[test]
    fn serde_transparent() {
        let r = EntityLabelRef::from_static("ssn");
        let s = serde_json::to_string(&r).unwrap();
        assert_eq!(s, "\"ssn\"");
        let back: EntityLabelRef = serde_json::from_str(&s).unwrap();
        assert_eq!(back, r);
    }
}
