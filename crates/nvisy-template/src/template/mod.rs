//! The [`Template`] value every template constructor returns:
//! the [`PolicyDefinition`]s + [`LabelGroup`]s a caller submits
//! together, plus identity metadata the engine records on every
//! run driven by this template.

pub(crate) mod ccpa;
pub(crate) mod gdpr;
pub(crate) mod hipaa;
pub(crate) mod pci;

use hipstr::HipStr;
use jiff::civil::Date;
use nvisy_policy::redaction::{ModalityRedactions, TextRedaction};
use nvisy_policy::{LabelGroup, PolicyDefinition};
use semver::Version;

/// A regulatory posture packaged as engine-ready data.
///
/// Templates are plain data; a caller wanting to diverge from
/// the shipped defaults mutates the returned [`policies`] /
/// [`groups`] before submitting. The engine never sees the
/// [`Template`] itself — only its [`policies`] and [`groups`]
/// via `Engine::analyze` / `Engine::anonymize`.
///
/// # Identity
///
/// Templates carry their own [`version`] and [`effective_date`],
/// distinct from the crate's release version and the individual
/// [`PolicyDefinition::id`]s inside. That separation matters
/// because regulatory text updates on its own cadence: a
/// customer pinned to `hipaa_safe_harbor` v1 doesn't want a
/// point-release of `nvisy-template` shifting the labelset out
/// from under them.
///
/// [`effective_date`]: Self::effective_date
/// [`groups`]: Self::groups
/// [`policies`]: Self::policies
/// [`version`]: Self::version
/// [`PolicyDefinition::id`]: nvisy_policy::PolicyDefinition::id
#[derive(Debug, Clone)]
pub struct Template {
    /// Stable identifier for the template, matched to the
    /// constructor that produced it (`"hipaa_safe_harbor"`,
    /// `"gdpr_article_9"`, ...). Not a display string —
    /// snake_case, ASCII, kebab-safe. Used to key registries and
    /// to name the template in audit annotations.
    pub name: HipStr<'static>,
    /// Semver-tracked version of *this* template, distinct from
    /// the crate version. A change to the shipped labelset or
    /// operator dispatch bumps this field. See [`semver::Version`]
    /// for the parse / comparison semantics.
    pub version: Version,
    /// The date the regulatory text this template encodes became
    /// effective. Not the date the template was authored — the
    /// date the *rule* took force. Reviewers reading an audit
    /// trail check this against the run date to confirm the
    /// template that fired was the one in force at the time.
    pub effective_date: Date,
    /// Human-readable name for reviewers. `name` is the machine
    /// identifier; this is the string a customer sees.
    pub description: HipStr<'static>,
    /// [`LabelGroup`]s the template's [`policies`] reference by
    /// name. Passed as-is to `Engine::analyze` / `anonymize`.
    /// May be empty for templates whose rules address labels
    /// directly (e.g. PAN-only templates).
    ///
    /// [`policies`]: Self::policies
    /// [`LabelGroup`]: nvisy_policy::LabelGroup
    pub groups: Vec<LabelGroup>,
    /// The [`PolicyDefinition`]s a caller submits in precedence
    /// order. Every template ships at least one.
    ///
    /// [`PolicyDefinition`]: nvisy_policy::PolicyDefinition
    pub policies: Vec<PolicyDefinition>,
}

/// Wrap a text-modality [`TextRedaction`] in a
/// [`ModalityRedactions`] with every other modality slot left
/// empty. The common case across every shipped template — text
/// is where the regulatory action lives.
pub(crate) fn text_action(spec: TextRedaction) -> ModalityRedactions {
    ModalityRedactions {
        text: Some(spec),
        ..Default::default()
    }
}
