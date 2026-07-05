# Nvisy Runtime

[![Runtime](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/rs-build.yml?branch=main&label=runtime&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/rs-build.yml)
[![Inference](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/py-build.yml?branch=main&label=inference&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/py-build.yml)

Multimodal redaction library and its inference services.

A workspace that pairs a stateless Rust redaction pipeline with the
BentoML-hosted Python model services it calls into. The Rust side ships
as library crates hosts embed directly; the Python side ships as Docker
containers hosts deploy alongside.

> [!WARNING]
> **Active development: API not stable.** This project is under active
> development. Public APIs, configuration shapes, and wire schemas may
> change without notice between releases. Pin a specific commit if you
> depend on this in production.

## Workspaces

Rust crates ([`crates/`](crates/)) — library only, no long-running
process. Hosts (a SaaS backend, a Tauri app, a language SDK, a custom
pipeline) embed the engine directly.

- **[nvisy-context](crates/nvisy-context/)**: wire schema for reference-data collections (analytic, biometric, document, geospatial, reference, temporal)
- **[nvisy-policy](crates/nvisy-policy/)**: wire schema for redaction governance (rules, predicates, retention, audit)
- **[nvisy-schema](crates/nvisy-schema/)**: umbrella crate re-exporting `nvisy-context` + `nvisy-policy` alongside plan and file types
- **[nvisy-core](crates/nvisy-core/)**: deployment-side runtime configuration (NER and LLM recognizer lineups, error vocabulary)
- **[nvisy-engine](crates/nvisy-engine/)**: stateless pipeline (decode, analyze, apply) wrapping elide and hosting the per-modality orchestrator
- **[elide-bento](crates/elide-bento/)**: BentoML-hosted NER and OCR client implementing elide's recognizer traits

Python packages ([`packages/`](packages/)) — BentoML services shipped
as Docker containers, called over HTTP by `elide-bento`.

- **[nvisy-core](packages/nvisy-core/)**: shared Python types and runtime helpers
- **[nvisy-ner](packages/nvisy-ner/)**: GLiNER-based named-entity recognition
- **[nvisy-ocr](packages/nvisy-ocr/)**: docTR-based detection OCR (text plus word-level geometry)
- **[nvisy-vl](packages/nvisy-vl/)**: PaddleOCR-VL vision-language OCR (high-accuracy transcription and layout)

## Features

- **Multimodal codecs**: read, edit, and write PDF, DOCX, images, audio, CSV, JSON, and plain text through a unified span-based content model
- **Layered detection**: regex, dictionary, and checksum patterns run first at low cost; NER, OCR, VLM, and LLM classification handle what deterministic methods cannot
- **Context-aware redaction**: mask, replace, hash, encrypt, blur, block, and pixelate with policy-driven rules scoped to entity type, document class, and confidence threshold
- **Stateless engine**: no persistence, no HTTP layer, no background tasks; every analyze and apply call is self-contained
- **Bring your own inference**: any service that reproduces the wire contract is a drop-in replacement for the shipped Python packages, including self-hosted or custom models and weights

## Quick Start

The fastest way to get started is with [Nvisy Cloud](https://nvisy.com).

For self-hosted use, embed the engine crate directly and deploy the
Python services as sidecar containers. See each crate and package
README for details.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
