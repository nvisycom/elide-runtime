# nvisy-engine

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

The Nvisy redaction pipeline: takes imported content through
extraction → detection → deduplication → redaction → validation →
export, generic over modality and parallel across documents.

## Overview

`ingestion::*` decodes incoming content into a typed
`DocumentEnvelope<M>` per source (text, tabular, image, audio).
`AnyEnvelope` is the modality-erased enum that lets a single run
carry envelopes of mixed modality.

`extraction::*` populates each envelope's blocks: OCR for image
documents, STT for audio, no-op for already-structured text and
tabular. Each technique is built once at startup from its
`[extractor.*]` config section and lives on the `ExtractionEngine`
registry; per-modality dispatch goes through the `Extract<M>` trait.

`detection::*` is the recognizer-side machinery:

- A modality-typed `Recognizer<Modality, Context>` trait with
  built-in adapters — `PatternRecognizer`, `NerRecognizer`,
  `LlmNerPipeline` (text), and `VlmPipeline` (image).
- `DetectionEngine` holds those recognizers as named slots, split
  into `TextRecognizers` (pattern is always-on; `llm`/`ner` are
  `Option`) and `ImageRecognizers` (`vlm` is `Option`), built once
  from `[detection.*]` config and shared via `Arc` across runs.
- Per-run dispatch parallelises every present slot via `JoinSet`,
  filtered by the plan's `Detection.kinds` allowlist — text blocks
  fan their `scan_text` to every selected text recognizer; image
  envelopes fan each image location to every selected image
  recognizer.
- `Detection::into_engine()` assembles a per-run engine from the
  registry, picking recognizers by `RecognizerKind`.

`deduplication::*`, `redaction::*`, `validation::*` are the
subsequent pipeline phases (merge overlapping detections, apply
the policy-driven redaction strategy per modality, optionally
re-scan the redacted output for leaks).

`pipeline::*` is the orchestrator: `DocumentPipeline<M>` runs one
envelope through every phase, monomorphised per modality.
`RuntimeConfig` is the top-level config struct that gathers
`[engine]`, `[extraction.*]`, `[detection.*]`, and `[redaction]`.

`envelope::*` defines the `DocumentEnvelope<M>` carrier itself
plus shared per-run state (run ID, policy store, registry).

## Feature Flags

Modality features enable the matching codec/format/extraction
arms; provider features select LLM/OCR backends. All four
modalities are on by default; provider features are opt-in.

| Feature | Default | Description |
|---------|---------|-------------|
| `tabular` | yes | CSV + XLSX |
| `image` | yes | PNG, JPEG, TIFF + OCR + VLM detection |
| `audio` | yes | WAV, MP3 + STT extraction |
| `rich` | yes | PDF, DOCX (pulls `image`) |
| `openai` | no | OpenAI providers (GPT, Whisper STT) |
| `anthropic` | no | Anthropic Claude completion provider |
| `google` | no | Google Gemini completion provider |
| `bento` | no | Externalised inference backends (BentoML NER + OCR) |
| `test-utils` | no | Test scaffolding (in-memory `Engine`) |

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
