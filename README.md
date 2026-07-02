# Nvisy Runtime

[![Rust Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/rs-build.yml?branch=main&label=rust&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/rs-build.yml)
[![Python Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/py-build.yml?branch=main&label=python&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/py-build.yml)

Multimodal redaction library and its inference services.

A workspace that pairs a stateless Rust redaction pipeline with the
BentoML-hosted Python model services it calls into. The Rust side
ships as library crates hosts embed directly; the Python side ships as
Docker containers hosts deploy alongside.

> [!WARNING]
> **Active development: API not stable.** This project is under active
> development. Public APIs, configuration shapes, and wire schemas may
> change without notice between releases. Pin a specific commit if you
> depend on this in production.

## Workspaces

### Rust crates (`crates/`)

Four library crates, no long-running process. Hosts (a SaaS backend, a
Tauri app, a language SDK, a custom pipeline) embed the engine
directly and layer whatever workflow, persistence, and multi-tenancy
they need on top.

- **nvisy-schema** — wire types (policy, context, plan, file).
  Consumed by SDKs on both sides of the HTTP boundary.
- **nvisy-core** — deployment-side runtime configuration (NER and LLM
  recognizer lineups, error vocabulary).
- **nvisy-engine** — stateless pipeline: decode, analyze, apply. Wraps
  elide and hosts the per-modality orchestrator.
- **elide-bento** — BentoML-hosted NER and OCR client implementing
  elide's recognizer traits.

### Python packages (`packages/`)

BentoML services that ship as Docker images. `elide-bento` calls them
over HTTP.

- **nvisy-core** — shared Python types and runtime helpers.
- **nvisy-ner** — GLiNER-based named-entity recognition.
- **nvisy-ocr** — docTR-based detection OCR (text plus word-level
  geometry).
- **nvisy-vl** — PaddleOCR-VL vision-language OCR (high-accuracy
  transcription and layout).

## Features

- **Multimodal codecs**: read, edit, and write PDF, DOCX, images,
  audio, CSV, JSON, and plain text through elide's unified span-based
  content model.
- **Layered detection**: regex, dictionary, and checksum patterns run
  first at low cost; NER, OCR, VLM, and LLM classification handle what
  deterministic methods cannot.
- **Context-aware redaction**: mask, replace, hash, encrypt, blur,
  block, and pixelate, with policy-driven rules scoped to entity
  type, document class, and confidence threshold.
- **Stateless**: the Rust engine holds no persistence, no HTTP layer,
  no background tasks. Every analyze and apply call is
  self-contained.
- **Bring your own inference**: any service that reproduces the wire
  contract is a drop-in replacement for the shipped Python packages,
  including self-hosted or custom models and weights.

## Quick Start

The fastest way to get started is with Nvisy Cloud (<https://nvisy.com>).

For self-hosted use, embed the engine crate directly and deploy the
Python services as sidecar containers. See each crate and package
README for details.

## Changelog

See CHANGELOG.md for release notes and version history.

## License

Apache 2.0 License, see LICENSE.txt.

## Support

- Documentation: <https://docs.nvisy.com>
- Issues: <https://github.com/nvisycom/runtime/issues>
- Email: support@nvisy.com
