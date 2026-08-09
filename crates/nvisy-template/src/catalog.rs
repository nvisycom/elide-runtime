//! [`TemplateCatalog`]: a registry of [`Template`]s a runtime
//! serves to callers.
//!
//! Runtime lookup and wire manifest in one type. A deployment
//! constructs the catalog once (typically from
//! [`TemplateCatalog::builtin`] plus caller-added extras),
//! queries it by [`Template::id`] and [`Template::version`], and
//! serialises it to JSON when a discovery API needs to expose
//! the shipped set.
//!
//! Keyed by `(id, version)` so multiple versions of the same
//! regulatory posture coexist — a customer transitioning between
//! HIPAA Safe Harbor revisions can hold `v1` and `v2` in one
//! catalog and pin per document class.

use std::collections::BTreeMap;
use std::sync::Arc;

use elide_core::{Error, ErrorKind, Result};
use hipstr::HipStr;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

use super::Template;

/// Runtime registry of [`Template`]s, keyed by
/// `(id, version)`. Templates are stored behind [`Arc`] so
/// [`get`] / [`latest`] / [`versions_of`] / [`iter`] return
/// cheap cloneable handles without deep-cloning a
/// `PolicyDefinition` chain.
///
/// # Serialisation
///
/// Serialises as a flat `Vec<Template>` on the wire (via
/// [`serde`]'s `from`/`into` shim) so discovery endpoints emit
/// `[template, template, ...]` rather than a nested map keyed
/// by tuple. Deserialising the same JSON rebuilds the
/// `(id, version)` index. Two templates with the same
/// `(id, version)` deserialise as last-wins.
///
/// The `into = "Vec<Template>"` serialize shim moves each stored
/// template out of the catalog when it can and deep-clones when
/// an external `Arc<Template>` holder prevents that (returned by
/// [`get`] / [`latest`] / [`versions_of`] / [`iter`]). Because
/// discovery endpoints commonly hold catalog handles concurrently
/// with serve requests, plan on the clone cost when embedding
/// large catalogs mid-request; use [`iter`] + hand-serialize when
/// the extra clone matters.
///
/// [`Arc`]: std::sync::Arc
/// [`get`]: Self::get
/// [`iter`]: Self::iter
/// [`latest`]: Self::latest
/// [`versions_of`]: Self::versions_of
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(from = "Vec<Template>", into = "Vec<Template>")]
pub struct TemplateCatalog {
    templates: BTreeMap<TemplateKey, Arc<Template>>,
}

/// Composite key `(id, version)` for [`TemplateCatalog`]'s
/// internal `BTreeMap`. Uses [`BTreeMap`] so iteration order is
/// deterministic (alphabetical by id, semver-ascending inside
/// each id) — reviewers reading [`TemplateCatalog::iter`] see a
/// stable order.
///
/// `Hash` is derived so a caller can swap the internal map for a
/// `HashMap` variant without changing this file — the composite
/// works in either.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TemplateKey {
    id: HipStr<'static>,
    version: Version,
}

impl TemplateCatalog {
    /// Empty catalog. Grow via [`insert`].
    ///
    /// [`insert`]: Self::insert
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The regulatory templates this crate ships:
    /// [`hipaa_safe_harbor`], [`gdpr_article_9`],
    /// [`pci_dss_pan_truncate`], [`pci_dss_pan_hmac`], [`ccpa`].
    /// A deployment starts here and extends with any custom
    /// templates via [`insert`].
    ///
    /// [`ccpa`]: crate::ccpa
    /// [`gdpr_article_9`]: crate::gdpr_article_9
    /// [`hipaa_safe_harbor`]: crate::hipaa_safe_harbor
    /// [`insert`]: Self::insert
    /// [`pci_dss_pan_hmac`]: crate::pci_dss_pan_hmac
    /// [`pci_dss_pan_truncate`]: crate::pci_dss_pan_truncate
    #[must_use]
    pub fn builtin() -> Self {
        let mut catalog = Self::new();
        for build in super::BUILTIN {
            // The shipped templates validate by construction —
            // insert only fails on caller-authored templates with
            // malformed ids.
            catalog
                .insert(build())
                .expect("shipped templates carry valid ids");
        }
        catalog
    }

    /// Insert `template`. If another template already occupies
    /// the same `(id, version)` slot, the old one is replaced.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Configuration`] when [`Template::id`]
    /// is empty or contains any character outside the
    /// `[a-z0-9_]` snake_case charset (the same shape
    /// [`Template::id`]'s docstring promises).
    pub fn insert(&mut self, template: Template) -> Result<()> {
        validate_id(&template.id)?;
        let key = TemplateKey {
            id: template.id.clone(),
            version: template.version.clone(),
        };
        self.templates.insert(key, Arc::new(template));
        Ok(())
    }

    /// Look up an exact `(id, version)` pair. `None` when no
    /// template with that combination is registered.
    #[must_use]
    pub fn get(&self, id: &str, version: &Version) -> Option<Arc<Template>> {
        self.templates
            .get(&TemplateKey {
                id: HipStr::from(id),
                version: version.clone(),
            })
            .cloned()
    }

    /// Latest version of a template by `id`. `None` when the
    /// `id` is unknown. Ties broken by [`semver::Version`]'s
    /// [`Ord`] impl (semver-natural, prerelease < release).
    #[must_use]
    pub fn latest(&self, id: &str) -> Option<Arc<Template>> {
        self.versions_of(id).next_back()
    }

    /// Every version registered under `id`, semver-ascending.
    /// Empty iterator when `id` is unknown.
    pub fn versions_of<'a>(
        &'a self,
        id: &'a str,
    ) -> impl DoubleEndedIterator<Item = Arc<Template>> + 'a {
        self.templates
            .iter()
            .filter(move |(key, _)| key.id.as_str() == id)
            .map(|(_, template)| template.clone())
    }

    /// Every template in the catalog, id-ascending and (within
    /// each id) semver-ascending.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = Arc<Template>> + '_ {
        self.templates.values().cloned()
    }

    /// Number of `(id, version)` slots registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// Whether the catalog has zero registered templates.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// Whether `id` has any version registered.
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.templates.keys().any(|key| key.id.as_str() == id)
    }
}

/// Reject a [`Template::id`] the [`TemplateCatalog`] docstring
/// promises to hold to: non-empty, ASCII, snake_case
/// (`[a-z0-9_]` only, and the first character must be
/// `[a-z]`). Matches how elide's builtin [`LabelRef`]s are
/// spelled — a shared convention across the two catalogs so
/// authors don't have to keep two rules in their heads.
///
/// [`LabelRef`]: elide_core::entity::LabelRef
fn validate_id(id: &str) -> Result<()> {
    let mut chars = id.chars();
    let first = chars.next().ok_or_else(|| {
        Error::new(
            ErrorKind::Configuration,
            "template catalog: `id` must be non-empty",
        )
    })?;
    if !first.is_ascii_lowercase() {
        return Err(Error::new(
            ErrorKind::Configuration,
            format!(
                "template catalog: `id` must start with a lowercase ASCII letter; \
                 `{id}` starts with `{first}`",
            ),
        ));
    }
    for c in chars {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return Err(Error::new(
                ErrorKind::Configuration,
                format!(
                    "template catalog: `id` must be snake_case `[a-z0-9_]`; \
                     `{id}` contains `{c}`",
                ),
            ));
        }
    }
    Ok(())
}

impl From<Vec<Template>> for TemplateCatalog {
    /// Deserialize path. Templates with malformed ids are
    /// silently dropped — deserialize can't return `Result` from
    /// a `From` impl, and the alternative (panic) is worse than
    /// dropping a bad row from a wire payload. Callers who need
    /// strict rejection use [`TemplateCatalog::insert`] directly
    /// with an already-parsed `Vec<Template>`.
    fn from(templates: Vec<Template>) -> Self {
        let mut catalog = Self::new();
        for template in templates {
            let _ = catalog.insert(template);
        }
        catalog
    }
}

impl From<TemplateCatalog> for Vec<Template> {
    /// Serialize path. Moves each stored `Template` out of its
    /// `Arc` when possible and deep-clones when another handle is
    /// live (see the [`TemplateCatalog`] docstring for the
    /// concurrency note).
    fn from(catalog: TemplateCatalog) -> Self {
        catalog
            .templates
            .into_values()
            .map(|arc| Arc::try_unwrap(arc).unwrap_or_else(|arc| (*arc).clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_ships_every_registered_template() {
        let catalog = TemplateCatalog::builtin();
        for build in super::super::BUILTIN {
            let id = build().id;
            assert!(
                catalog.contains(&id),
                "builtin catalog must carry `{id}` from the BUILTIN registry",
            );
        }
        assert_eq!(catalog.len(), super::super::BUILTIN.len());
    }

    #[test]
    fn latest_returns_the_highest_version() {
        let mut catalog = TemplateCatalog::new();
        let mut v1 = crate::hipaa_safe_harbor();
        v1.version = Version::new(1, 0, 0);
        let mut v2 = crate::hipaa_safe_harbor();
        v2.version = Version::new(2, 0, 0);
        let mut v1_1 = crate::hipaa_safe_harbor();
        v1_1.version = Version::new(1, 1, 0);
        catalog.insert(v1).unwrap();
        catalog.insert(v2).unwrap();
        catalog.insert(v1_1).unwrap();
        let latest = catalog.latest("hipaa_safe_harbor").expect("id registered");
        assert_eq!(latest.version, Version::new(2, 0, 0));
    }

    #[test]
    fn get_returns_the_exact_version_or_none() {
        let mut catalog = TemplateCatalog::new();
        let mut v1 = crate::gdpr_article_9();
        v1.version = Version::new(1, 0, 0);
        let mut v2 = crate::gdpr_article_9();
        v2.version = Version::new(2, 0, 0);
        catalog.insert(v1).unwrap();
        catalog.insert(v2).unwrap();
        assert!(
            catalog
                .get("gdpr_article_9", &Version::new(1, 0, 0))
                .is_some()
        );
        assert!(
            catalog
                .get("gdpr_article_9", &Version::new(2, 0, 0))
                .is_some()
        );
        assert!(
            catalog
                .get("gdpr_article_9", &Version::new(3, 0, 0))
                .is_none()
        );
        assert!(
            catalog
                .get("no_such_template", &Version::new(1, 0, 0))
                .is_none()
        );
    }

    #[test]
    fn versions_of_yields_semver_ascending() {
        let mut catalog = TemplateCatalog::new();
        let mut v2 = crate::ccpa();
        v2.version = Version::new(2, 0, 0);
        let mut v1 = crate::ccpa();
        v1.version = Version::new(1, 0, 0);
        // Insert out of order.
        catalog.insert(v2).unwrap();
        catalog.insert(v1).unwrap();
        let versions: Vec<Version> = catalog
            .versions_of("ccpa")
            .map(|t| t.version.clone())
            .collect();
        assert_eq!(versions, vec![Version::new(1, 0, 0), Version::new(2, 0, 0)]);
    }

    #[test]
    fn insert_same_key_replaces_the_existing_entry() {
        let mut catalog = TemplateCatalog::new();
        let mut first = crate::hipaa_safe_harbor();
        first.name = "First".into();
        let mut second = crate::hipaa_safe_harbor();
        second.name = "Second".into();
        catalog.insert(first).unwrap();
        catalog.insert(second).unwrap();
        assert_eq!(catalog.len(), 1);
        let latest = catalog.latest("hipaa_safe_harbor").unwrap();
        assert_eq!(latest.name.as_str(), "Second");
    }

    #[test]
    fn insert_rejects_empty_id() {
        let mut catalog = TemplateCatalog::new();
        let mut template = crate::hipaa_safe_harbor();
        template.id = "".into();
        let err = catalog
            .insert(template)
            .expect_err("empty id must be rejected");
        assert!(err.to_string().contains("non-empty"));
    }

    #[test]
    fn insert_rejects_uppercase_and_hyphen_ids() {
        let mut catalog = TemplateCatalog::new();
        let mut template = crate::hipaa_safe_harbor();
        template.id = "HIPAA".into();
        catalog
            .insert(template)
            .expect_err("uppercase id must be rejected");

        let mut kebab = crate::hipaa_safe_harbor();
        kebab.id = "hipaa-safe-harbor".into();
        catalog
            .insert(kebab)
            .expect_err("hyphenated id must be rejected");

        let mut leading_digit = crate::hipaa_safe_harbor();
        leading_digit.id = "1hipaa".into();
        catalog
            .insert(leading_digit)
            .expect_err("id starting with digit must be rejected");
    }

    #[test]
    fn insert_accepts_snake_case_with_digits() {
        let mut catalog = TemplateCatalog::new();
        let mut template = crate::hipaa_safe_harbor();
        template.id = "hipaa_v2_1".into();
        catalog
            .insert(template)
            .expect("snake_case with digits is valid");
    }

    #[test]
    fn serde_round_trip_preserves_full_template_contents() {
        let original = TemplateCatalog::builtin();
        let json = serde_json::to_string(&original).expect("serialize");
        // Wire format is a flat array (not a keyed map).
        assert!(json.starts_with('['));
        let round: TemplateCatalog = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(round.len(), original.len());
        for template in original.iter() {
            let recovered = round
                .get(&template.id, &template.version)
                .expect("every original template must round-trip");
            // Full structural equality, not just the composite key
            // — catches any field that fails to (de)serialize.
            assert_eq!(recovered.id, template.id);
            assert_eq!(recovered.name, template.name);
            assert_eq!(recovered.version, template.version);
            assert_eq!(recovered.effective_date, template.effective_date);
            assert_eq!(recovered.description, template.description);
            let round_policies = serde_json::to_value(&recovered.policies).unwrap();
            let orig_policies = serde_json::to_value(&template.policies).unwrap();
            assert_eq!(
                round_policies, orig_policies,
                "template `{}` policies must round-trip byte-identical",
                template.id,
            );
        }
    }
}
