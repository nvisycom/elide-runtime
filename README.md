# Nvisy Runtime

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Multimodal redaction library for sensitive data.

A workspace of Rust library crates that detect and remove sensitive
information across documents, images, and audio. Combines deterministic
patterns, NER, vision-language model classification, and whole-audit
LLM verification into auditable, policy-driven pipelines built for
regulated industries such as healthcare, legal, government, and
financial services.

> [!WARNING]
> **Active development: API not stable.** This project is under active
> development. Public APIs, configuration shapes, and wire schemas may
> change without notice between releases. Pin a specific commit if you
> depend on this in production.

## Workspace

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
- **elide-bento** — BentoML-hosted NER and OCR backends implementing
  elide's recognizer traits.

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
- **Stateless**: no persistence, no HTTP layer, no background tasks.
  Every analyze and apply call is self-contained.

## Quick Start

The fastest way to get started is with Nvisy Cloud (<https://nvisy.com>).

For self-hosted use, embed the engine crate directly. See each crate's
README for details.

## Changelog

See CHANGELOG.md for release notes and version history.

## License

Apache 2.0 License, see LICENSE.txt.

## Support

- Documentation: <https://docs.nvisy.com>
- Issues: <https://github.com/nvisycom/runtime/issues>
- Email: support@nvisy.com
