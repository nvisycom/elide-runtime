# nvisy-engine

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

DAG compiler and executor for the Nvisy runtime — compiles workflow
definitions into executable pipelines, manages run lifecycles, and
orchestrates detection + redaction across modalities.

## Overview

`workflow::*` defines the graph-config shape (ingest → extraction →
detection → refinement → context → policy → export, plus
cross-cutting policy types). `pipeline::*` compiles a workflow into
an executable `Pipeline`, owns the per-run `RuntimeConfig`, and
drives execution via `Pipeline::execute`. `operation::*` hosts the
concrete operation implementations each graph node maps to —
`Detection`, `Extraction`, `Deduplication`, redaction
strategies, etc.

`detection::*` is the recognizer-side machinery: the trait surface
(re-exported from `nvisy_core::detection::Recognizer`), the
parallel-dispatching `DetectionEngine`, and the built-in
recognizers (`PatternRecognizer`, `NerRecognizer`, `LlmRecognizer`).
`Detection::into_engine()` auto-assembles a `DetectionEngine` from
workflow config — one recognizer per opted-in slot.

`registry::*` holds the workflow-node type registry; `utility::*`
ships compression and encryption helpers used by the pipeline.

## Feature Flags

Vendor features control which LLM and OCR providers are compiled in.
All are disabled by default; the CLI/server entry points enable
them.

| Feature | Default | Description |
|---------|---------|-------------|
| `openai` | no | Enable all OpenAI providers (GPT, Whisper STT) |
| `anthropic` | no | Enable Anthropic Claude completion provider |
| `google` | no | Enable Google Gemini + Google Cloud Vision OCR |
| `microsoft` | no | Enable Azure Document Intelligence OCR |
| `amazon` | no | Enable AWS Textract OCR |

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
