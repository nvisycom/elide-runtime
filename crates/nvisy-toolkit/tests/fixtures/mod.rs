//! Shared test fixtures for the toolkit's codec E2E tests
//! (`codec_e2e_txt.rs`, `codec_e2e_json.rs`, `codec_e2e_csv.rs`,
//! `codec_e2e_html.rs`).
//!
//! Each integration test declares `mod fixtures;` and pulls the
//! pieces it needs from the re-exports below. The boilerplate of
//! building the recognizer + redaction registries, driving the
//! four-stage `decode → detect → dedup → redact + encode`
//! pipeline, and asserting on the result lives here so the
//! per-codec tests stay focused on entity-kind coverage and
//! codec-specific structural checks.
//!
//! Submodules:
//!
//! - `pipeline` — the [`Fixture`] descriptor, the per-modality
//!   pipeline drivers, the [`PipelineOutcome`] return type, and
//!   the `*.redacted.*` artifact writer.
//! - `registries` — the shipped recognizer + redaction registry +
//!   dedup params every test uses.
//! - `asserts` — entity-presence and PII-removed/token-present
//!   helpers.

// Each integration-test file pulls a different subset of the
// re-exports below, so the un-used ones look "dead" from any single
// test's perspective. Quiet both lints across the whole module.
#![allow(dead_code, unused_imports)]

mod asserts;
mod pipeline;
mod registries;

pub use self::asserts::{
    assert_pii_removed, assert_tabular_entity, assert_text_entity, assert_tokens_present,
};
pub use self::pipeline::{Fixture, PipelineOutcome};
pub use self::registries::{dedup_params, redaction_registry, shipped_recognizer};
