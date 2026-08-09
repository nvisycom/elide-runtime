#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! ## Architecture
//!
//! [`PolicyTemplate`] enumerates the regulatory postures this
//! crate ships. Variants that admit more than one operator
//! choice the regulation permits carry a small options struct
//! (e.g. [`PolicyTemplate::PciDssPan`] carries a [`PciPanRender`]
//! picking truncate-vs-HMAC — both listed under PCI DSS §3.5.1);
//! variants with a single shipped operator take no data.
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
pub use self::template::{HipaaDeidMethod, PciPanRender, Template};
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
    },
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
    /// (PAN) unreadable. `render` picks between the truncation
    /// and keyed-HMAC postures §3.5.1 permits. Targets the
    /// elide-builtin `payment_card` label; no [`LabelGroup`]
    /// (one label, one operator).
    ///
    /// [`LabelGroup`]: nvisy_policy::LabelGroup
    PciDssPan {
        /// Which of the §3.5.1-permitted render approaches to
        /// apply. See [`PciPanRender`] for the tradeoff.
        render: PciPanRender,
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
            Self::HipaaDeidentification { method } => hipaa::template(method),
            Self::GdprArticle9 => gdpr::template(),
            Self::PciDssPan { render } => pci::template(render),
            Self::Ccpa => ccpa::template(),
        }
    }
}
