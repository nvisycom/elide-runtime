# nvisy-document

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Whole-document carrier and pipeline orchestrator for the Nvisy
runtime — owns `Document<M>`, the per-modality block / metadata
shapes, the policy store, the audit trail, the ingestion surface, and
the per-document phase orchestrators that drive a document through
extraction → detection → deduplication → redaction → validation.

## Overview

Depends on `nvisy-core` for the atoms (primitives, `Entity`,
`Modality`, `ValueAt`, the `ModalityExtraction` enums) and on
`nvisy-toolkit` for the composable components the phases call
into (`RecognizerRegistry`, `ExtractionEngine`, `LayerPipeline`,
`CheckPipeline`, redaction strategies). Document is the only place
where the toolkit-shape and document-shape types meet.

`document::*` holds the typed carrier: `Document<M>`, `Block<M>`,
`Span<M>`, and the `Audit` record that accumulates `EntityRecord`s as
detection runs. The carrier is generic over `M: DocumentModality`, the
extension trait declared in [`modality`](src/modality/mod.rs) that
binds `Block` and `Metadata` per modality (`TextBlock` / `TextMetadata`
for `Text`, `ImageBlock` / `ImageMetadata` for `Image`, …).

`modality::*` re-exports `Text` / `Image` / `Audio` / `Tabular` from
core, defines `DocumentModality` and `ModalityBlock`, and exposes the
per-modality block + metadata structs. The `TextBlock::Embed` variant
wraps an `EmbeddedDocument` (currently `Image` only) so nested
documents — PDF page images, embedded figures — sit inside their
parent text flow without losing per-modality typing.

`core::*` is the per-run plumbing: `SharedData` (engine-shared
registry + key provider + policy store), `RunContext` (per-run
cancellation token + Arc to `SharedData`), `DocumentTree` /
`NodeMut` (the per-run document carrier the orchestrator walks),
`PolicyStore` (heterogeneous `type_map` of `Vec<Arc<Policy<M>>>`),
and `DocumentView<'_, M>` / `SharedHandle` (the `ValueAt<M>` impl
that resolves a location back to its source text via the codec
handle).

`phases::*` hosts the per-document phase orchestrators. Each phase is
a document-walking driver around its toolkit-side subsystem:
`ExtractionPhase` around `nvisy_toolkit::extraction::ExtractionEngine`,
`DetectionPhase` around `nvisy_toolkit::detection::RecognizerRegistry`,
`DeduplicationPhase` around `nvisy_toolkit::deduplication::LayerPipeline`,
`RedactionPhase` around the codec apply path, `ValidationPhase` around
`nvisy_toolkit::validation::CheckPipeline`, plus `ingestion::Importer`
/ `Exporter` and the on-disk `Registry`. Toolkit subsystems stay free
of `Document<M>` knowledge so they can be exercised standalone; phases
are the only place toolkit- and document-shape types meet.

`pipeline::*` is the runtime: `Engine` (the long-lived service holding
the registry and pre-built engines), `Pipeline` (per-run lifecycle —
resource acquisition, execution, retention, finalisation),
`DocumentPipeline` (the per-document phase sequence), `Orchestrator`
(fan-in over imported documents). The `config::*` submodule hosts the
TOML-deserialised `RuntimeConfig` + `EngineConfig` + per-phase plan
nodes; per-section deployment configs (`ExtractionConfig`,
`DetectionConfig`, `RedactionConfig`, `DeduplicationParams`) live in
the toolkit and are re-exported here.

`policy::*` carries the strategy-policy types: `Policy<M>` itself,
`PolicyRule<M>`, `Action::Redact { strategy }` / `Action::Suppress`,
`Condition`, `RetentionPolicy`, and the `RetentionScope` enum that
drives the retention enforcement at run finalisation.

`provenance::*` is the per-entity audit: `EntityRecord`, `AuditEntry`,
and the `RedactionMap` of per-modality replacement values produced by
the redaction phase.

`validation::*` owns the `Check<M>` trait, the `CheckPipeline` that
composes checks, and the canonical `LeakCheck` implementation that
re-scans redacted content for leftover entities.

## Feature Flags

Modality features control which per-modality surface is compiled in;
backend-flavour flags forward to provider crates.

| Feature | Default | Description |
|---------|---------|-------------|
| `tabular` | yes | Tabular-modality codec + formats |
| `image` | yes | Image-modality codec + formats |
| `audio` | yes | Audio-modality codec + formats |
| `rich` | yes | Rich-document codec (PDF/DOCX); pulls `image` |
| `openai` | no | OpenAI providers (GPT, Whisper) |
| `anthropic` | no | Anthropic Claude provider |
| `google` | no | Google Gemini provider |
| `bento` | no | Externalised BentoML backends (NER, OCR) |
| `test-utils` | no | In-memory helpers used by the integration tests |

## Documentation

See [`docs/`](../../docs/) for architecture, security, and API documentation.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
