//! The [`Template`] value every template constructor returns:
//! the [`PolicyDefinition`]s a caller submits, plus identity
//! metadata the engine records on every run driven by this
//! template.

pub(crate) mod ccpa;
pub(crate) mod gdpr;
pub(crate) mod hipaa;
pub(crate) mod pci;

use elide_core::entity::audit::AttributionKind;
use elide_governance::{PolicyDefinition, TemplateOrigin};
use hipstr::HipStr;
use jiff::civil::Date;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::{Uuid, uuid};

pub use self::gdpr::{GdprArticle9Treatment, GdprSensitiveScope};
pub use self::hipaa::{HipaaAccountNumbers, HipaaDeidMethod};
pub use self::pci::{PciDssPart, PciPanRender};

/// A regulatory posture packaged as engine-ready data.
///
/// Templates are plain data; a caller wanting to diverge from
/// the shipped defaults mutates the returned [`policy`]
/// before submitting. The engine never sees the [`Template`]
/// itself: only its [`policy`] via `Engine::analyze` /
/// `Engine::anonymize`. The policy carries its own
/// [`LabelScope`]s inline via [`PolicyDefinition::scopes`].
///
/// # Identity
///
/// Three separate identity fields, mirroring how elide's
/// [`Label`] separates identity from display:
///
/// - [`id`]: the machine key (`"hipaa_safe_harbor"`,
///   snake_case, ASCII, kebab-safe). Stable across template
///   version bumps. What audits, registries, and API paths key
///   on.
/// - [`name`]: the short display string
///   (`"HIPAA Safe Harbor de-identification"`). What a customer
///   sees in a UI or a picker.
/// - [`description`]: optional longer prose for reviewers.
///
/// Plus [`version`] and [`effective_date`]:
///
/// - [`version`]: semver-tracked version of *this* template,
///   distinct from the crate's release version. A change to the
///   shipped labelset or operator dispatch bumps this field.
///   Multiple versions of the same [`id`] can coexist: a customer
///   transitioning between regulatory revisions might hold `v1` and
///   `v2` at once and pin per document class.
/// - [`effective_date`]: the date the regulatory text this
///   template encodes became effective (not the date the
///   template was authored). Reviewers reading an audit trail
///   check this against the run date to confirm the template
///   that fired was the one in force at the time.
///
/// [`Label`]: elide_core::entity::Label
/// [`LabelScope`]: elide_governance::LabelScope
/// [`PolicyDefinition`]: elide_governance::PolicyDefinition
/// [`PolicyDefinition::scopes`]: elide_governance::PolicyDefinition::scopes
/// [`description`]: Self::description
/// [`effective_date`]: Self::effective_date
/// [`id`]: Self::id
/// [`name`]: Self::name
/// [`policy`]: Self::policy
/// [`version`]: Self::version
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    /// Machine key: snake_case, ASCII, kebab-safe. Stable
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
    /// its own [`LabelScope`]s inline. A caller composing
    /// several regulatory postures in one request submits
    /// multiple templates and unions their policies into the
    /// engine's `&[PolicyDefinition]` slice.
    ///
    /// [`LabelScope`]: elide_governance::LabelScope
    /// [`PolicyDefinition`]: elide_governance::PolicyDefinition
    pub policy: PolicyDefinition,
}

/// A [`Cited`] attribution for a rule or group.
///
/// Shorthand for the struct-variant literal, which every shipped
/// template writes and which reads poorly inline at the call
/// sites.
///
/// [`Cited`]: AttributionKind::Cited
pub(crate) fn cited(
    authority: &'static str,
    citation: &'static str,
    rationale: &'static str,
) -> AttributionKind {
    AttributionKind::Cited {
        authority: HipStr::borrowed(authority),
        citation: HipStr::borrowed(citation),
        rationale: HipStr::borrowed(rationale),
    }
}

/// The [`TemplateOrigin`] a shipped template stamps on its policy.
///
/// Every shipped template is at `1.0.0`; the version is passed
/// explicitly so a template that bumps its own revision records
/// the new one here without the call site drifting from
/// [`Template::version`].
pub(crate) fn origin(id: &'static str, version: Version) -> TemplateOrigin {
    TemplateOrigin::new(HipStr::borrowed(id), version)
}

/// Namespace for every identity this crate derives.
///
/// A fixed, arbitrary UUID: v5 guarantees that the same
/// `(namespace, name)` always yields the same UUID, so ids stay
/// stable across builds, and a private namespace keeps them from
/// colliding with any other v5 scheme a deployment runs.
const TEMPLATE_NAMESPACE: Uuid = uuid!("018f5a2c-0000-7000-8000-000000000000");

/// A stable UUID derived from `name`.
///
/// Configuration axes that change a template's emitted labels must
/// change its identity too: `accounts` and `scope` alter which
/// labels a policy covers, and an audit keyed on a shared UUID
/// could not tell the coverages apart. Feeding every axis into the
/// name means each distinct posture gets a distinct, reproducible
/// identity without hand-assigning a constant per combination.
pub(crate) fn derived_id(name: &str) -> Uuid {
    Uuid::new_v5(&TEMPLATE_NAMESPACE, name.as_bytes())
}
