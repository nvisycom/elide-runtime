//! [`Fixture`] descriptor + the per-modality pipeline drivers every
//! codec E2E test runs as methods on the fixture.
//!
//! Each driver wires the shared recognizer + redaction registries
//! into a `decode → detect → dedup → redact + encode` flow and
//! writes the redacted output next to the fixture as
//! `{stem}.redacted.{ext}` for human inspection.

use std::str::from_utf8;

use nvisy_codec::{CodecRegistry, DocumentHandle};
use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Modality, Tabular, Text};
use nvisy_core::redaction::{RedactAt, Redactions};
use nvisy_toolkit::deduplication::{LayerContext, LayerPipeline};
use nvisy_toolkit::detection::{RecognizerRegistry, RecognizerRegistryExt};

use super::registries::{dedup_params, redaction_registry, shipped_recognizer};

/// Fixture file the codec E2E tests load. `path` is the absolute
/// disk path the artifact-writer drops `{stem}.redacted.{ext}`
/// next to; `source` is the compile-time-inlined fixture body;
/// `extension` is the codec hint the registry resolves on.
pub struct Fixture {
    /// Absolute path to the fixture on disk; used by the artifact
    /// writer to derive the `{stem}.redacted.{ext}` output path.
    pub path: &'static str,
    /// Fixture body (the bytes the codec decodes).
    pub source: &'static str,
    /// Extension hint the codec registry resolves on.
    pub extension: &'static str,
}

impl Fixture {
    /// Decode this fixture into a typed text [`DocumentHandle`].
    pub async fn decode_text(&self) -> Result<DocumentHandle<Text>> {
        let registry = CodecRegistry::with_builtin();
        let untyped = registry.decode(self.source, self.extension).await?;
        Ok(untyped
            .into_text()
            .expect("text-modality extension resolves to text handle"))
    }

    /// Decode this fixture into a typed tabular [`DocumentHandle`].
    pub async fn decode_tabular(&self) -> Result<DocumentHandle<Tabular>> {
        let registry = CodecRegistry::with_builtin();
        let untyped = registry.decode(self.source, self.extension).await?;
        Ok(untyped
            .into_tabular()
            .expect("tabular-modality extension resolves to tabular handle"))
    }

    /// Run the full text pipeline: decode → detect-per-chunk →
    /// dedup → redact + encode → write `*.redacted.*` artifact.
    pub async fn run_text_pipeline(&self) -> PipelineOutcome<Text> {
        let mut buffer = self.decode_text().await.expect("text fixture decodes");

        let recognizer_registry = RecognizerRegistry::new().with_recognizer(shipped_recognizer());
        let detected = recognizer_registry
            .detect(buffer.handler_mut())
            .await
            .expect("text detect succeeds");

        let ctx = LayerContext::<Text, _>::new(&buffer);
        let entities = LayerPipeline::<Text, _>::from_params(&dedup_params())
            .expect("pipeline builds")
            .run(detected, &ctx)
            .await;

        let redactions: Redactions<Text> = redaction_registry::<Text>()
            .apply_all(entities.iter(), &buffer)
            .await
            .expect("text redactions apply");
        buffer
            .redact_at(redactions)
            .await
            .expect("text redact succeeds");

        let encoded = buffer.handler().encode().expect("text encode succeeds");
        let redacted = from_utf8(encoded.as_bytes())
            .expect("text codec encode produces UTF-8")
            .to_owned();

        self.write_redacted_artifact(&redacted);
        PipelineOutcome { entities, redacted }
    }

    /// Run the full tabular pipeline. Same shape as
    /// [`run_text_pipeline`] but for `M = Tabular`.
    ///
    /// [`run_text_pipeline`]: Self::run_text_pipeline
    pub async fn run_tabular_pipeline(&self) -> PipelineOutcome<Tabular> {
        let mut buffer = self
            .decode_tabular()
            .await
            .expect("tabular fixture decodes");

        let recognizer_registry = RecognizerRegistry::new().with_recognizer(shipped_recognizer());
        let detected = recognizer_registry
            .detect(buffer.handler_mut())
            .await
            .expect("tabular detect succeeds");

        let ctx = LayerContext::<Tabular, _>::new(&buffer);
        let entities = LayerPipeline::<Tabular, _>::from_params(&dedup_params())
            .expect("pipeline builds")
            .run(detected, &ctx)
            .await;

        let redactions: Redactions<Tabular> = redaction_registry::<Tabular>()
            .apply_all(entities.iter(), &buffer)
            .await
            .expect("tabular redactions apply");
        buffer
            .redact_at(redactions)
            .await
            .expect("tabular redact succeeds");

        let encoded = buffer.handler().encode().expect("tabular encode succeeds");
        let redacted = from_utf8(encoded.as_bytes())
            .expect("tabular codec encode produces UTF-8")
            .to_owned();

        self.write_redacted_artifact(&redacted);
        PipelineOutcome { entities, redacted }
    }

    /// Write `redacted` next to the fixture as `{stem}.redacted.{ext}`
    /// for human inspection. Gitignored under
    /// `**/testdata/**/*.redacted.*`.
    fn write_redacted_artifact(&self, redacted: &str) {
        let path = std::path::Path::new(self.path);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("fixture has a UTF-8 stem");
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .expect("fixture has a UTF-8 extension");
        let parent = path.parent().expect("fixture has a parent");
        let out = parent.join(format!("{stem}.redacted.{ext}"));
        std::fs::write(&out, redacted).unwrap_or_else(|e| {
            panic!("write redacted artifact {}: {e}", out.display());
        });
    }
}

/// Outcome of one end-to-end pipeline run. Tests pull both the
/// entity list (to assert detection coverage) and the encoded
/// redacted body (to assert structural preservation + token
/// presence).
pub struct PipelineOutcome<M: Modality> {
    /// Entities surviving dedup (the input to redact).
    pub entities: Vec<Entity<M>>,
    /// UTF-8 encoded redacted output.
    pub redacted: String,
}
