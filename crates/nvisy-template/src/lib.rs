#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! ## Architecture
//!
//! [`PolicyTemplate`] enumerates the regulatory postures this
//! crate ships. [`PolicyTemplate::build`] materialises the
//! picked variant into a [`Template`] — the [`PolicyDefinition`]
//! carrying its own inline [`LabelGroup`]s — matched to how the
//! engine consumes them. Callers hand `template.policy` (as a
//! one-element slice via [`std::slice::from_ref`]) to
//! `Engine::analyze` / `Engine::anonymize`, or compose several
//! templates' policies into one slice when they want more than
//! one regulatory posture per request.
//!
//! Five variants across four regulatory postures:
//! [`PolicyTemplate::HipaaSafeHarbor`],
//! [`PolicyTemplate::GdprArticle9`],
//! [`PolicyTemplate::PciDssPanTruncate`],
//! [`PolicyTemplate::PciDssPanHmac`], [`PolicyTemplate::Ccpa`].
//!
//! Templates are plain data. Nothing is registered globally, and
//! no template constructor talks to the engine or hits I/O. A
//! caller wanting to diverge from the shipped operator (say,
//! swap the default [`TextRedaction::Erase`] for a
//! [`TextRedaction::Pseudonymize`] for retained analytics use)
//! mutates the returned [`PolicyDefinition`] before submitting.
//!
//! Every [`Template`] carries a machine [`Template::id`]
//! (snake_case) distinct from its display [`Template::name`],
//! mirroring how elide's `Label` splits identity from display.
//! Plus its own [`Version`] and the [`Date`] the regulatory
//! text became effective, distinct from the crate's release
//! version — a customer that must pin to a snapshot pins
//! [`Template::version`], not the crate version.
//!
//! [`TemplateCatalog`] wraps every shipped template in a
//! `(id, version)`-keyed registry. Serve one from a discovery
//! endpoint via its serde derives; look one up by id at runtime
//! via [`TemplateCatalog::latest`] or
//! [`TemplateCatalog::get`]. [`TemplateCatalog::builtin`]
//! returns a catalog seeded with every [`PolicyTemplate`]
//! variant.
//!
//! [`Date`]: jiff::civil::Date
//! [`LabelGroup`]: nvisy_policy::LabelGroup
//! [`PolicyDefinition`]: nvisy_policy::PolicyDefinition
//! [`TextRedaction::Erase`]: nvisy_policy::redaction::TextRedaction::Erase
//! [`TextRedaction::Pseudonymize`]: nvisy_policy::redaction::TextRedaction::Pseudonymize
//! [`Version`]: semver::Version

mod catalog;
mod template;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

pub use self::catalog::TemplateCatalog;
pub use self::template::Template;
use self::template::{ccpa, gdpr, hipaa, pci};

/// A regulatory posture this crate ships a [`Template`] for.
///
/// Serialises as a snake_case string matching the produced
/// template's [`Template::id`] (`"hipaa_safe_harbor"`,
/// `"gdpr_article_9"`, ...) so a wire caller can round-trip
/// `template: "hipaa_safe_harbor"` through JSON directly into
/// a variant. Iterate every variant via `PolicyTemplate::iter()`
/// (from [`strum::IntoEnumIterator`]).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PolicyTemplate {
    /// HIPAA Safe Harbor de-identification per 45 CFR
    /// §164.514(b)(2). Ships a `hipaa_18` [`LabelGroup`] naming
    /// the 18 identifier categories the rule enumerates, plus a
    /// [`TableRule`] dispatching each identifier to the operator
    /// called for by the safe-harbor posture (ages ≥90
    /// [`Clamp`]ed, dates [`GeneralizeDate`]d to the year, the
    /// remainder [`TextRedaction::Erase`]d).
    ///
    /// [`Clamp`]: nvisy_policy::redaction::TextRedaction::Clamp
    /// [`GeneralizeDate`]: nvisy_policy::redaction::TextRedaction::GeneralizeDate
    /// [`LabelGroup`]: nvisy_policy::LabelGroup
    /// [`TableRule`]: nvisy_policy::TableRule
    /// [`TextRedaction::Erase`]: nvisy_policy::redaction::TextRedaction::Erase
    HipaaSafeHarbor,
    /// GDPR Article 9 special categories of personal data. Ships
    /// a `gdpr_article_9` [`LabelGroup`] naming the nine special
    /// categories the article enumerates, plus a [`Predicated`]
    /// rule using [`Predicate::LabelInGroup`] to erase every
    /// match.
    ///
    /// [`LabelGroup`]: nvisy_policy::LabelGroup
    /// [`Predicate::LabelInGroup`]: nvisy_policy::predicate::Predicate::LabelInGroup
    /// [`Predicated`]: nvisy_policy::PolicyRule::Predicated
    GdprArticle9,
    /// PCI DSS §3.5.1 — render stored Primary Account Numbers
    /// (PAN) unreadable via truncation to the first-six /
    /// last-four digits. Targets the elide-builtin `payment_card`
    /// label through [`TextRedaction::Truncate`] with
    /// `keep_prefix: 6, keep_suffix: 4`. No [`LabelGroup`] —
    /// one label, one operator.
    ///
    /// [`LabelGroup`]: nvisy_policy::LabelGroup
    /// [`TextRedaction::Truncate`]: nvisy_policy::redaction::TextRedaction::Truncate
    PciDssPanTruncate,
    /// PCI DSS §3.5.1 — render stored PAN unreadable via a
    /// keyed HMAC hash. Requires the engine to have a
    /// `KeyProvider` wired (see `Engine::with_key_provider`).
    PciDssPanHmac,
    /// CCPA "personal information" categories per Cal. Civ.
    /// Code §1798.140(v). Ships a `ccpa_personal_information`
    /// [`LabelGroup`] naming the enumerated categories, plus a
    /// [`PolicyDefinition`] using [`Predicate::LabelInGroup`]
    /// to erase every match. Customers commonly override the
    /// operator to [`TextRedaction::Pseudonymize`] where the
    /// retained data drives analytics.
    ///
    /// [`LabelGroup`]: nvisy_policy::LabelGroup
    /// [`PolicyDefinition`]: nvisy_policy::PolicyDefinition
    /// [`Predicate::LabelInGroup`]: nvisy_policy::predicate::Predicate::LabelInGroup
    /// [`TextRedaction::Pseudonymize`]: nvisy_policy::redaction::TextRedaction::Pseudonymize
    Ccpa,
}

impl PolicyTemplate {
    /// Materialise this variant into a fresh [`Template`]. Each
    /// call returns byte-identical templates (stable UUIDs baked
    /// as constants).
    #[must_use]
    pub fn build(self) -> Template {
        match self {
            Self::HipaaSafeHarbor => hipaa::template(),
            Self::GdprArticle9 => gdpr::template(),
            Self::PciDssPanTruncate => pci::truncate_template(),
            Self::PciDssPanHmac => pci::hmac_template(),
            Self::Ccpa => ccpa::template(),
        }
    }
}
