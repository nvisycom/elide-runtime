#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! ## Architecture
//!
//! One function per shipped template, each returning a
//! self-contained [`Template`] — the [`PolicyDefinition`]s each
//! carrying its own inline [`LabelGroup`]s — matched to how the
//! engine consumes them. Callers hand `template.policy`
//! (as a one-element slice via `std::slice::from_ref`) to
//! `Engine::analyze` / `Engine::anonymize`, or compose several
//! templates' policies into one slice when they want more than
//! one regulatory posture per request.
//!
//! Five templates across four regulatory postures:
//! [`hipaa_safe_harbor`], [`gdpr_article_9`],
//! [`pci_dss_pan_truncate`], [`pci_dss_pan_hmac`], [`ccpa`].
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
//! returns a catalog seeded with the five shipped templates.
//!
//! [`Date`]: jiff::civil::Date
//! [`LabelGroup`]: nvisy_policy::LabelGroup
//! [`PolicyDefinition`]: nvisy_policy::PolicyDefinition
//! [`TextRedaction::Erase`]: nvisy_policy::redaction::TextRedaction::Erase
//! [`TextRedaction::Pseudonymize`]: nvisy_policy::redaction::TextRedaction::Pseudonymize
//! [`Version`]: semver::Version

mod catalog;
mod template;

pub use self::catalog::TemplateCatalog;
pub use self::template::Template;
use self::template::{ccpa, gdpr, hipaa, pci};

/// HIPAA Safe Harbor de-identification per 45 CFR §164.514(b)(2).
///
/// Ships a `hipaa_18` [`LabelGroup`] naming the 18 identifier
/// categories the rule enumerates, plus one [`PolicyDefinition`]
/// whose [`TableRule`] dispatches each identifier to the
/// operator called for by the safe-harbor posture (ages ≥90
/// [`Clamp`]ed, dates [`GeneralizeDate`]d to the year, the
/// remainder [`TextRedaction::Erase`]d).
///
/// [`Clamp`]: nvisy_policy::redaction::TextRedaction::Clamp
/// [`GeneralizeDate`]: nvisy_policy::redaction::TextRedaction::GeneralizeDate
/// [`LabelGroup`]: nvisy_policy::LabelGroup
/// [`PolicyDefinition`]: nvisy_policy::PolicyDefinition
/// [`TableRule`]: nvisy_policy::TableRule
/// [`TextRedaction::Erase`]: nvisy_policy::redaction::TextRedaction::Erase
#[must_use]
pub fn hipaa_safe_harbor() -> Template {
    hipaa::template()
}

/// GDPR Article 9 special categories of personal data.
///
/// Ships a `gdpr_article_9` [`LabelGroup`] naming the nine
/// special categories the article enumerates, plus one
/// [`PolicyDefinition`] with a [`Predicated`] rule using
/// [`Predicate::LabelInGroup`] to erase every match.
///
/// [`LabelGroup`]: nvisy_policy::LabelGroup
/// [`Predicate::LabelInGroup`]: nvisy_policy::predicate::Predicate::LabelInGroup
/// [`Predicated`]: nvisy_policy::PolicyRule::Predicated
/// [`PolicyDefinition`]: nvisy_policy::PolicyDefinition
#[must_use]
pub fn gdpr_article_9() -> Template {
    gdpr::template()
}

/// PCI DSS §3.5.1 — render stored Primary Account Numbers (PAN)
/// unreadable via truncation to the first-six / last-four digits.
///
/// Ships one [`PolicyDefinition`] whose sole rule dispatches
/// the elide-builtin `payment_card` label through
/// [`TextRedaction::Truncate`] with `keep_prefix: 6, keep_suffix: 4`.
/// No [`LabelGroup`] — one label, one operator.
///
/// [`LabelGroup`]: nvisy_policy::LabelGroup
/// [`PolicyDefinition`]: nvisy_policy::PolicyDefinition
/// [`TextRedaction::Truncate`]: nvisy_policy::redaction::TextRedaction::Truncate
#[must_use]
pub fn pci_dss_pan_truncate() -> Template {
    pci::truncate_template()
}

/// PCI DSS §3.5.1 — render stored PAN unreadable via a keyed
/// HMAC hash. Requires the engine to have a `KeyProvider` wired.
///
/// Same shape as [`pci_dss_pan_truncate`] with
/// [`TextRedaction::HmacHash`] in the operator slot.
///
/// [`TextRedaction::HmacHash`]: nvisy_policy::redaction::TextRedaction::HmacHash
#[must_use]
pub fn pci_dss_pan_hmac() -> Template {
    pci::hmac_template()
}

/// CCPA "personal information" categories per Cal. Civ. Code
/// §1798.140(v).
///
/// Ships a `ccpa_personal_information` [`LabelGroup`] naming the
/// enumerated categories, plus one [`PolicyDefinition`] using
/// [`Predicate::LabelInGroup`] to erase every match. Customers
/// commonly override the operator to [`TextRedaction::Pseudonymize`]
/// where the retained data drives analytics.
///
/// [`LabelGroup`]: nvisy_policy::LabelGroup
/// [`Predicate::LabelInGroup`]: nvisy_policy::predicate::Predicate::LabelInGroup
/// [`PolicyDefinition`]: nvisy_policy::PolicyDefinition
/// [`TextRedaction::Pseudonymize`]: nvisy_policy::redaction::TextRedaction::Pseudonymize
#[must_use]
pub fn ccpa() -> Template {
    ccpa::template()
}

/// Every template this crate ships, as constructor pointers.
/// Consumed by [`TemplateCatalog::builtin`] to seed a fresh
/// catalog; not public because [`TemplateCatalog`] is the
/// discovery / lookup surface — callers who want the shipped set
/// go through it (`TemplateCatalog::builtin()`), and callers who
/// want one template call the constructor by name directly
/// ([`hipaa_safe_harbor`] etc.).
pub(crate) const BUILTIN: &[fn() -> Template] = &[
    hipaa_safe_harbor,
    gdpr_article_9,
    pci_dss_pan_truncate,
    pci_dss_pan_hmac,
    ccpa,
];
