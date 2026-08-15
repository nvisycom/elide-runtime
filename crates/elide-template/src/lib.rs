#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Layers on top of the [elide] toolkit. This crate adds
//! ready-to-run [`PolicyDefinition`]s for common regulatory
//! postures (HIPAA §164.514, GDPR Article 9, PCI DSS, CCPA / CPRA)
//! so callers submit a template instead of authoring the
//! governance surface by hand.
//!
//! [elide]: https://github.com/nvisycom/elide
//!
//! ## Architecture
//!
//! [`PolicyTemplate`] enumerates the regulatory postures this
//! crate ships. Variants that admit more than one shipped
//! posture carry a small options enum — e.g.
//! [`PolicyTemplate::PciDss`] carries a [`PciDssPart`] picking
//! between §3.5.1 PAN render (with a nested [`PciPanRender`]
//! choice) and §3.3.1 SAV erasure;
//! [`PolicyTemplate::GdprArticle9`] carries a
//! [`GdprArticle9Treatment`] picking erasure vs.
//! pseudonymization for the Article 9(2) lawful-basis case.
//!
//! [`PolicyTemplate::build`] materialises the picked variant
//! into a [`Template`] — the [`PolicyDefinition`] carrying its
//! own inline [`LabelGroup`]s — matched to how the engine
//! consumes it. Callers hand `template.policy` (as a
//! one-element slice via [`std::slice::from_ref`]) to
//! `Engine::analyze` / `Engine::anonymize`, or compose several
//! templates' policies into one slice when they want more than
//! one regulatory posture per request.
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
//! [`Date`]: jiff::civil::Date
//! [`LabelGroup`]: elide_governance::LabelGroup
//! [`PolicyDefinition`]: elide_governance::PolicyDefinition
//! [`TextRedaction::Erase`]: elide_governance::redaction::TextRedaction::Erase
//! [`TextRedaction::Pseudonymize`]: elide_governance::redaction::TextRedaction::Pseudonymize
//! [`Version`]: semver::Version

mod template;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::template::{
    GdprArticle9, GdprArticle9Treatment, GdprSensitiveScope, HipaaAccountNumbers, HipaaDeidMethod,
    HipaaDeidentification, PciDssPart, PciPanRender, Template,
};
use self::template::{ccpa, pci};

/// A regulatory posture this crate ships a [`Template`] for.
///
/// Serialises as an internally-tagged object under `kind`,
/// matching the house pattern for option-bearing enums
/// (`Predicate`, `TextRedaction`, `AnyRedaction`). A caller
/// wire-picks a template as
/// `{"kind": "hipaa_safe_harbor"}` or, for variants with
/// operator options,
/// `{"kind": "pci_dss_pan", "render": "hmac_sha256"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PolicyTemplate {
    /// HIPAA §164.514 de-identification. See
    /// [`HipaaDeidentification`] for the method + account-tier
    /// axes.
    HipaaDeidentification(HipaaDeidentification),
    /// GDPR Article 9 special categories of personal data,
    /// optionally widened with Recital 26 re-identification
    /// hardening (`date_of_birth`, `postal_code`) or Article 10
    /// criminal-justice labels (`criminal_record`,
    /// `criminal_charge`, `judicial_narrative`). See
    /// [`GdprArticle9`] for the config axes.
    GdprArticle9(GdprArticle9),
    /// PCI DSS. `part` picks between §3.5.1 stored-PAN render
    /// postures (with a nested [`PciPanRender`] choice) and
    /// §3.3.1 Sensitive Authentication Data erasure.
    PciDss {
        /// Which DSS subsection this template addresses. See
        /// [`PciDssPart`] for the shipped subsections.
        part: PciDssPart,
    },
    /// CCPA "personal information" categories per Cal. Civ.
    /// Code §1798.140(v). Ships a `ccpa_personal_information`
    /// [`LabelGroup`] naming the enumerated categories, plus a
    /// [`PolicyDefinition`] using [`Predicate::LabelInGroup`]
    /// to erase every match. Customers commonly override the
    /// operator to [`TextRedaction::Pseudonymize`] where the
    /// retained data drives analytics.
    ///
    /// [`LabelGroup`]: elide_governance::LabelGroup
    /// [`PolicyDefinition`]: elide_governance::PolicyDefinition
    /// [`Predicate::LabelInGroup`]: elide_governance::Predicate::LabelInGroup
    /// [`TextRedaction::Pseudonymize`]: elide_governance::redaction::TextRedaction::Pseudonymize
    Ccpa,
}

impl PolicyTemplate {
    /// Materialise this variant into a fresh [`Template`]. Each
    /// call returns byte-identical templates (stable UUIDs baked
    /// as constants).
    #[must_use]
    pub fn build(self) -> Template {
        match self {
            Self::HipaaDeidentification(cfg) => cfg.template(),
            Self::GdprArticle9(cfg) => cfg.template(),
            Self::PciDss { part } => pci::template(part),
            Self::Ccpa => ccpa::template(),
        }
    }
}
