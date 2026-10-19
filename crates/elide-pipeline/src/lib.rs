#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! Layers on top of the [elide] toolkit. This crate wires elide's
//! per-modality analyzers, anonymizers, and orchestrator into a
//! stateless per-request document pipeline, driven by its own
//! request schemas ([`RequestContext`](provider::RequestContext), [`file`](mod@file)) and the
//! governance schema from [elide-schema].
//!
//! [elide]: https://github.com/nvisycom/elide
//! [elide-schema]: https://docs.rs/elide-schema
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
//! [`LabelScope`]: elide_schema::policy::LabelScope
//! [`Policy::scopes`]: elide_schema::policy::Policy::scopes
//!
//! [`elide`]: elide

mod audit;
mod engine;
pub mod entity;
pub mod file;

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
/// [`LanguageClaim`](primitive::LanguageClaim) — but neither
/// should have to depend on elide directly to say so.
pub mod primitive {
    /// How a PDF is sampled when its pages are rasterised.
    ///
    /// From the PDF codec rather than the shared primitives: it
    /// configures that codec alone, and only a build carrying it
    /// can honour a non-default mode.
    #[cfg(feature = "codec-pdf")]
    #[cfg_attr(docsrs, doc(cfg(feature = "codec-pdf")))]
    #[doc(inline)]
    pub use elide::codec::pdf::RasterMode;
    /// The geometry and time primitives a modality location is
    /// built from: an image bounding box and polygon, a point, an
    /// audio time span.
    #[doc(inline)]
    pub use elide::primitive::{BoundingBox, Point, Polygon, TimeSpan};
    /// How a request describes its content: the country a document
    /// is from, and each claim about the language it is in.
    #[doc(inline)]
    pub use elide::primitive::{CountryCode, LanguageClaim};
}

/// What a pass spent, call by call.
///
/// Grouped for the same reason as [`primitive`]: these hang off the
/// entries on [`Audit::usage`] rather than appearing on a signature
/// of this crate's own, so they belong behind a name instead of
/// crowding the root.
///
/// Always recorded: every configured backend is wrapped, so an
/// audit says what each model call cost whether or not a
/// deployment bills on it. A pass that called no model carries an
/// empty tally, which is skipped on the wire.
///
/// [`Audit::usage`]: Audit::usage
pub mod usage {
    /// One backend call's record, and the pieces it is built from:
    /// the model called, whether the call succeeded, and the units
    /// it spent — tokens, audio seconds, pages, images, characters.
    ///
    /// [`UsageCollector`] is the sink a run collects into, and
    /// [`Metered`] the decorator that feeds it; a host wiring its
    /// own accounting needs both.
    #[doc(inline)]
    pub use elide::backend::metrics::{
        Metered, ModelUsage, Outcome, Usage, UsageCollector, UsageSink,
    };
    /// Which component a usage entry belongs to: its configured
    /// name and the version that ran.
    ///
    /// Covers enrichers as well as recognizers, so one identity
    /// spans both halves of a lineup.
    #[doc(inline)]
    pub use elide::primitive::ComponentId;
    /// The scope metadata a recognizer ran under.
    #[doc(inline)]
    pub use elide::recognition::ScopeMetadata;
}

/// One backend call's cost, carried on [`Audit::usage`].
///
/// Stays at the root, unlike the types it is built from: it is the
/// element type of a public field, so a signature over an [`Audit`]
/// has to name it. Its constituents live in [`usage`].
///
/// [`Audit::usage`]: Audit::usage
#[doc(inline)]
pub use elide::backend::metrics::Usage;
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

/// Providers: who does the detecting, and what a single request
/// carries into them.
///
/// Re-exported whole rather than name by name. The crate keeps its
/// internals private and re-exports a flat facade at its own root,
/// so there is no stutter to work around and no hand-maintained
/// import list here to drift out of step with it.
#[doc(inline)]
pub use elide_provider as provider;
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
pub use elide_schema as governance;
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

pub use self::audit::{
    Analyzed, Audit, RegisteredComponents, RegisteredEnricher, RegisteredRecognizer, Unhandled,
};
pub use self::engine::Engine;
