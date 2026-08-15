//! The [`Template`] value every template constructor returns:
//! the [`PolicyDefinition`]s a caller submits, plus identity
//! metadata the engine records on every run driven by this
//! template.

pub(crate) mod ccpa;
pub(crate) mod gdpr;
pub(crate) mod hipaa;
pub(crate) mod pci;
pub(crate) mod soc2;

use elide_governance::PolicyDefinition;
use hipstr::HipStr;
use jiff::civil::Date;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

pub use self::gdpr::{GdprArticle9, GdprArticle9Treatment, GdprSensitiveScope};
pub use self::hipaa::{HipaaAccountNumbers, HipaaDeidMethod, HipaaDeidentification};
pub use self::pci::{PciDssPart, PciPanRender};

/// A regulatory posture packaged as engine-ready data.
///
/// Templates are plain data; a caller wanting to diverge from
/// the shipped defaults mutates the returned [`policy`]
/// before submitting. The engine never sees the [`Template`]
/// itself — only its [`policy`] via `Engine::analyze` /
/// `Engine::anonymize`. The policy carries its own
/// [`LabelGroup`]s inline via [`PolicyDefinition::groups`].
///
/// # Identity
///
/// Three separate identity fields, mirroring how elide's
/// [`Label`] separates identity from display:
///
/// - [`id`] — the machine key (`"hipaa_safe_harbor"`,
///   snake_case, ASCII, kebab-safe). Stable across template
///   version bumps. What audits, registries, and API paths key
///   on.
/// - [`name`] — the short display string
///   (`"HIPAA Safe Harbor de-identification"`). What a customer
///   sees in a UI or a picker.
/// - [`description`] — optional longer prose for reviewers.
///
/// Plus [`version`] and [`effective_date`]:
///
/// - [`version`] — semver-tracked version of *this* template,
///   distinct from the crate's release version. A change to the
///   shipped labelset or operator dispatch bumps this field.
///   Multiple versions of the same [`id`] can coexist in a
///   [`TemplateCatalog`] simultaneously — a customer transitioning
///   between regulatory revisions might hold `v1` and `v2` at
///   once and pin per document class.
/// - [`effective_date`] — the date the regulatory text this
///   template encodes became effective (not the date the
///   template was authored). Reviewers reading an audit trail
///   check this against the run date to confirm the template
///   that fired was the one in force at the time.
///
/// [`Label`]: elide_core::entity::Label
/// [`LabelGroup`]: elide_governance::LabelGroup
/// [`PolicyDefinition`]: elide_governance::PolicyDefinition
/// [`PolicyDefinition::groups`]: elide_governance::PolicyDefinition::groups
/// [`TemplateCatalog`]: super::TemplateCatalog
/// [`description`]: Self::description
/// [`effective_date`]: Self::effective_date
/// [`id`]: Self::id
/// [`name`]: Self::name
/// [`policy`]: Self::policy
/// [`version`]: Self::version
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    /// Machine key — snake_case, ASCII, kebab-safe. Stable
    /// across version bumps; audits and registries key on this.
    #[schemars(with = "String")]
    pub id: HipStr<'static>,
    /// Short human-readable display string.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Semver version. See [`semver::Version`] for parse /
    /// comparison semantics.
    #[schemars(with = "String")]
    pub version: Version,
    /// The date the regulatory text this template encodes became
    /// effective.
    #[schemars(with = "String")]
    pub effective_date: Date,
    /// Optional longer prose for reviewers. `None` when the
    /// short `name` says enough.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub description: Option<HipStr<'static>>,
    /// The [`PolicyDefinition`] this template encodes. Carries
    /// its own [`LabelGroup`]s inline. A caller composing
    /// several regulatory postures in one request submits
    /// multiple templates and unions their policies into the
    /// engine's `&[PolicyDefinition]` slice.
    ///
    /// [`LabelGroup`]: elide_governance::LabelGroup
    /// [`PolicyDefinition`]: elide_governance::PolicyDefinition
    pub policy: PolicyDefinition,
}
