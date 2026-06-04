# nvisy-toolkit

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Composable component library for Nvisy pipelines — the recognizer
registry, extraction engine, deduplication layers, validation checks,
and redaction strategies that a consumer plugs into their own
document-processing pipeline.

## Overview

Sits between `nvisy-core` (the atoms: primitives, `Entity`, the
`Modality` marker trait, `ModalityData`, `EntityRecognizer<M>`,
`ValueAt`, the `ModalityExtraction` extension trait) and
`nvisy-document` (the whole-document runner that wires these
components together against a `Document<M>` tree). Toolkit owns the
pieces; document owns the orchestration.

`detection::*` hosts the recognizer side: the per-modality
`Detection` / `DetectionConfig` config nodes, the per-recognizer
`NerDetection` / `PatternDetection` configs, and the
`RecognizerRegistry` that holds the built `Arc<dyn EntityRecognizer<M>>`
instances each phase fans entities out to.

`deduplication::*` is the layered post-detection pipeline. `Layer<M, R>`
is the trait every composable pass implements; built-in implementations
are `CalibrateLayer`, `FilterLayer`, `FuseLayer`, and
`ResolveConflictsLayer`. `LayerPipeline` orders them, and
`DeduplicationParams` is the deployment-time bundle the engine reads
from `[deduplication]` in `Nvisy.toml`.

`extraction::*` mirrors the detection shape on the producer side.
`Extractor<M: ModalityData + ModalityExtraction>` is the trait every
extractor backend implements — same shape as
`EntityRecognizer<M>`, with `type Output` for the extractor's
modality-specific return shape and `fn extraction() -> M::Extraction`
for the per-modality provenance value stamped on the document.
`ExtractionEngine` is the deployment-time registry holding optional
`Arc<OcrExtractor>` / `Arc<SttExtractor>` slots. The concrete
`OcrExtractor` (`Extractor<Image>`) wraps `nvisy_ocr::Extractor`;
`SttExtractor` (`Extractor<Audio>`) wraps
`nvisy_agent::audio::stt::SttService`.

`redaction::*` declares the redaction surface: the `Redactable`
extension trait that binds the per-modality `Replacement` record;
the `Anonymizer<M>` / `Deanonymizer<M>` operator traits; the
built-in operator structs (`Replace`, `Mask`, `Hash`, `Redact`,
`Keep`, optionally `Encrypt` / `Decrypt`); and the
`RedactionRegistry<M>` — a per-modality name-keyed pool of
deployment-supplied custom operators looked up by
`AnonymizerId<M>`. Built-ins never live in the registry; they're
instantiated per-call from the policy's operator-spec enum on the
document side.

## Feature Flags

Modality features control which per-modality surface is compiled in;
the bento and openai flags forward to backend crates.

| Feature | Default | Description |
|---------|---------|-------------|
| `tabular` | yes | Tabular-modality detection / dedup / redaction surface |
| `image` | yes | Image-modality surface plus `OcrExtractor` (pulls `nvisy-ocr`) |
| `audio` | yes | Audio-modality surface plus `SttExtractor` (pulls `nvisy-agent`) |
| `bento` | no | Externalised BentoML backends — forwards to `nvisy-ner/bento` and `nvisy-ocr/bento` |
| `openai` | no | OpenAI Whisper STT provider — forwards to `nvisy-agent/openai-whisper` |

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
