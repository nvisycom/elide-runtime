#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

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
//! [`TemplateCatalog`] wraps every shipped template in a
//! `(id, version)`-keyed registry. Serve one from a discovery
//! endpoint via its serde derives; look one up by id at runtime
//! via [`TemplateCatalog::latest`] or
//! [`TemplateCatalog::get`]. [`TemplateCatalog::builtin`]
//! returns a catalog seeded with every shipped
//! `(kind, options)` pairing.
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

pub use self::catalog::TemplateCatalog;
pub use self::template::{
    GdprArticle9Treatment, HipaaAccountNumbers, HipaaDeidMethod, PciDssPart, PciPanRender, Template,
};
use self::template::{ccpa, gdpr, hipaa, pci};

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
    /// HIPAA §164.514 de-identification. `method` picks between
    /// the fixed Safe Harbor rule set (§164.514(b)(2)), the
    /// narrower Limited Data Set subtraction (§164.514(e)(2))
    /// that keeps dates and coarse geography for DUA-governed
    /// research handoffs, and the Expert Determination scaffold
    /// (§164.514(b)(1)) for statistician-signed workflows.
    HipaaDeidentification {
        /// Which §164.514 method to apply. See [`HipaaDeidMethod`]
        /// for the tradeoff.
        method: HipaaDeidMethod,
        /// Which §(J) account-identifier labels to remove.
        /// Defaults to [`HipaaAccountNumbers::Standard`] (bank
        /// account + IBAN + payment card). Pick
        /// [`HipaaAccountNumbers::Extended`] to add crypto
        /// wallet addresses under the §(R) catch-all reading.
        #[serde(default)]
        accounts: HipaaAccountNumbers,
    },
    /// GDPR Article 9 special categories of personal data.
    /// `treatment` picks between erasure (the default no-basis
    /// posture) and pseudonymization (identity-preserving,
    /// requires an Article 9(2) lawful-basis carve-out
    /// established out-of-band).
    GdprArticle9 {
        /// Which operator to apply to Article 9 matches. See
        /// [`GdprArticle9Treatment`] for the tradeoff.
        treatment: GdprArticle9Treatment,
    },
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
            Self::HipaaDeidentification { method, accounts } => hipaa::template(method, accounts),
            Self::GdprArticle9 { treatment } => gdpr::template(treatment),
            Self::PciDss { part } => pci::template(part),
            Self::Ccpa => ccpa::template(),
        }
    }
}
