#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Layers on top of the [elide] toolkit. This crate wires elide's
//! per-modality analyzers, anonymizers, and orchestrator into a
//! stateless per-request document pipeline, driven by its own
//! request schemas ([`RequestContext`](provider::RequestContext), [`file`](mod@file)) and the
//! governance schema from [elide-governance].
//!
//! [elide]: https://github.com/nvisycom/elide
//! [elide-governance]: https://docs.rs/elide-governance
//!
//! ## Architecture
//!
//! Stateless redaction pipeline over [`elide`]. Bytes (or a
//! caller-owned path) go in, a detection report or redacted
//! bytes come out. No persistence, no HTTP, no long-running
//! background tasks. Hosts (a SaaS API, a Tauri app, a CLI, a
//! language SDK) embed this crate and layer whatever workflow,
//! storage, and multi-tenancy they need on top.
//!
//! - [`Engine`]: the entry point. Bundles the codec registry,
//!   the deployment's NER / LLM lineups, the shared
//!   [`KeyProvider`] for keyed operators (HMAC, AES), and the
//!   per-request orchestrator builder.
//! - [`Engine::analyze`] and [`Engine::anonymize`] compile and
//!   run the full [`elide::Orchestrator`] against one document.
//!   Both take the request's `policies` alongside the document;
//!   each policy carries its own [`LabelScope`]s inline via
//!   [`Policy::scopes`].
//! - [`Audit`] carries the analyze → anonymize handoff: the
//!   modality-tagged entity groups plus a
//!   [`DocumentContext`](provider::DocumentContext) with
//!   the request's asserted scope and correlation id.
//!
//! Ready-to-run policy sets for common regulatory postures
//! (HIPAA §164.514, GDPR Article 9, PCI DSS, CCPA / CPRA)
//! live in the sibling `elide-template` crate, re-exported here
//! as [`template`]. Each template carries a single
//! `Policy` (with inline [`LabelScope`]s) that a caller
//! hands to [`Engine::analyze`] / [`Engine::anonymize`] as a
//! one-element slice.
//!
//! [`LabelScope`]: elide_governance::policy::LabelScope
//! [`Policy::scopes`]: elide_governance::policy::Policy::scopes
//!
//! [`elide`]: elide

pub mod entity;
pub mod file;
mod pipeline;

#[doc(inline)]
pub use elide::codec::FormatRegistry;

/// The modality markers every typed call is generic over.
///
/// Reading an [`Audit`] means naming one:
/// `audit.report.entities::<Text>()`, `audit.review::<Image>(..)`.
///
/// All four are always available: the modalities compile in
/// unconditionally, and only their codecs are feature-gated.
pub mod modality {
    #[doc(inline)]
    pub use elide::modality::audio::Audio;
    #[doc(inline)]
    pub use elide::modality::image::Image;
    #[doc(inline)]
    pub use elide::modality::tabular::Tabular;
    #[doc(inline)]
    pub use elide::modality::text::Text;
    #[doc(inline)]
    pub use elide::modality::{Modality, ModalityLocation};
    /// Each medium's own vocabulary: its location and coordinate
    /// types, its data and replacement types.
    ///
    /// The marker types above name a modality; these say where
    /// something sits inside one — an [`image::ImageLocation`]'s
    /// bounding box, a [`text::TextLocation`]'s coordinate, an
    /// [`audio::AudioLocation`]'s time span, a
    /// [`tabular::TabularLocation`]'s row and column. A consumer
    /// addressing an entity needs the whole set.
    #[doc(inline)]
    pub use elide::modality::{audio, image, tabular, text};
}

/// The scalar types a location or a request is described with.
///
/// Grouped rather than re-exported flat: nothing here appears on
/// this crate's own signatures, they are reached *through* the
/// types that do. A consumer naming an
/// [`ImageLocation`](modality::image::ImageLocation) needs
/// [`BoundingBox`](primitive::BoundingBox) to read its geometry,
/// and one configuring a request needs
/// [`Languages`](primitive::Languages) — but neither should have
/// to depend on elide directly to say so.
pub mod primitive {
    /// The geometry and time primitives a modality location is
    /// built from: an image bounding box and polygon, a point, an
    /// audio time span.
    #[doc(inline)]
    pub use elide::primitive::{BoundingBox, Point, Polygon, TimeSpan};
    /// How a request describes its content: the country and
    /// languages a document is in, and how a raster is sampled.
    #[doc(inline)]
    pub use elide::primitive::{CountryCode, Languages, RasterMode};
}

/// What a pass spent, and which recognizer spent it.
///
/// Grouped for the same reason as [`primitive`]: these hang off
/// [`UsageReport`] — reached via [`Audit::usage`] — rather than
/// appearing on a signature of this crate's own, so they belong
/// behind a name instead of crowding the root.
///
/// [`UsageReport`]: UsageReport
/// [`Audit::usage`]: Audit::usage
pub mod usage {
    #[doc(inline)]
    pub use elide::recognition::{ModelUsage, RecognizerId, ScopeMetadata, TokenCounts, Usage};
}

/// The report of what a pass spent, carried on [`Audit::usage`].
///
/// Stays at the root, unlike the types it is built from: it is the
/// type of a public field, so a signature over an [`Audit`] has to
/// name it. Its constituents live in [`usage`].
///
/// [`Audit::usage`]: Audit::usage
#[doc(inline)]
pub use elide::recognition::UsageReport;
/// The encryption-key source a PCI-DSS posture requires, wired via
/// `Engine::with_key_provider`.
///
/// Stays at the root: a consumer *implements* this trait, so it is
/// an entry point to the engine rather than a detail reached
/// through another type.
#[doc(inline)]
pub use elide::redaction::operators::KeyProvider;
/// The two halves of what an analysis produced, re-exported so a
/// consumer can name what [`Analyzed`] hands it.
///
/// [`Report`] is the reference half — entities and their audit
/// trails, no content — and rides on [`Audit::report`].
/// [`ArtifactSet`] is the content half, the enrichment a pass
/// extracted, and rides on [`Analyzed::artifacts`]. Both are
/// public fields, so both types have to be nameable to write a
/// signature over them.
///
/// [`Audit::report`]: Audit::report
/// [`Analyzed::artifacts`]: Analyzed::artifacts
#[doc(inline)]
pub use elide::{ArtifactSet, Report};
pub use elide::{Error, ErrorKind, Result};

/// Rendering an [`Audit`] into a transport format.
///
/// Only what a caller needs to *export*: the traits and the table
/// selector. `elide_export`'s implementor-facing plumbing
/// (`write_rows`, `TableRows`) stays out — writing a new
/// `ExportCsv` impl means depending on that crate directly.
#[cfg(any(feature = "audit-csv", feature = "audit-json"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "audit-csv", feature = "audit-json"))))]
pub mod export {
    /// Whole-document export: blanket implemented for every
    /// [`Serialize`](serde::Serialize) type, so
    /// [`Audit`](crate::Audit) gains it from its own serialization.
    #[cfg(feature = "audit-json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "audit-json")))]
    #[doc(inline)]
    pub use elide_export::ExportJson;
    /// Flat-table export: [`Audit`](crate::Audit) projects into one
    /// CSV table per [`Table`] variant.
    ///
    /// The `audit-zip` feature adds `ExportCsv::write_zip`, bundling
    /// every table into a single archive.
    #[cfg(feature = "audit-csv")]
    #[cfg_attr(docsrs, doc(cfg(feature = "audit-csv")))]
    #[doc(inline)]
    pub use elide_export::{ExportCsv, Table};
}

/// Governance: what happens to sensitive data, and the vocabulary
/// a request introduces.
///
/// Re-exported under its own name rather than as `policy`. The
/// crate holds two halves — [`policy`](governance::policy) for the
/// rules and [`recognition`](governance::recognition) for the
/// vocabulary — so aliasing the whole thing to one of them both
/// stutters (`policy::policy::Policy`) and puts the vocabulary
/// behind a module that has nothing to do with it.
#[doc(inline)]
pub use elide_governance as governance;
/// Providers: who does the detecting, and what a single request
/// carries into them.
///
/// Re-exported whole rather than name by name. The crate keeps its
/// internals private and re-exports a flat facade at its own root,
/// so there is no stutter to work around and no hand-maintained
/// import list here to drift out of step with it.
#[doc(inline)]
pub use elide_provider as provider;
/// Templates: ready-to-run [`Policy`](governance::policy::Policy)
/// sets for common regulatory postures.
///
/// Re-exported whole, like [`provider`]: the crate keeps its
/// internals private and presents a flat facade at its own root,
/// so `template::Template` is the path either way and there is no
/// import list here to drift out of step with it. A caller picks a
/// posture (HIPAA, GDPR Article 9, PCI DSS, CCPA/CPRA) instead of
/// authoring the governance surface by hand.
#[doc(inline)]
pub use elide_template as template;

pub use self::pipeline::{
    Analyzed, Audit, Engine, RegisteredComponents, RegisteredEnricher, RegisteredRecognizer,
    Unhandled,
};
