# elide-runtime

[![Runtime](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=runtime&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)
[![Inference](https://img.shields.io/github/actions/workflow/status/nvisycom/bento/build.yml?branch=main&label=inference&style=flat-square)](https://github.com/nvisycom/bento/actions/workflows/build.yml)

Multimodal redaction pipeline as a stateless Rust library.

`elide-runtime` layers on top of the [elide](https://github.com/nvisycom/elide)
toolkit. Elide provides the low-level primitives — the recognizers,
anonymizers, and the tamper-evident audit log — and `elide-runtime`
wires them into a document-oriented pipeline with governance types,
regulatory policy templates, and a stateless engine. Use this crate
set alongside elide-core (which it depends on directly), not instead
of it.

A workspace of library crates hosts (a SaaS backend, a Tauri app, a
language SDK, a custom pipeline) embed directly. No long-running
process, no HTTP layer of its own. Inference is delegated over HTTP to
model services that live in the sibling
[nvisycom/bento](https://github.com/nvisycom/bento) repository; the
engine ships with a git-dep client (`elide-bento`) that speaks their
wire contract, and any service reproducing that contract is a drop-in
replacement.

## Crates

- `elide-wire` — wire schemas for plan and file (analyzer parameters, document envelope)
- `elide-governance` — wire schema for redaction governance (rules, predicates, operators)
- `elide-template` — ready-to-run policy templates (HIPAA, GDPR, PCI DSS, CCPA, SOC 2)
- `elide-pipeline` — stateless document pipeline (decode, analyze, apply)

> [!WARNING]
> **Active development: API not stable.** This project is under active
> development. Public APIs, configuration shapes, and wire schemas may
> change without notice between releases. Pin a specific commit if you
> depend on this in production.

## Features

- **Multimodal codecs**: read, edit, and write PDF, DOCX, images, audio, CSV, JSON, and plain text through a unified span-based content model
- **Layered detection**: regex, dictionary, and checksum patterns run first at low cost; NER, OCR, VLM, and LLM classification handle what deterministic methods cannot
- **Context-aware redaction**: mask, replace, hash, encrypt, blur, block, and pixelate with policy-driven rules scoped to entity type, document class, and confidence threshold
- **Stateless engine**: no persistence, no HTTP layer, no background tasks; every analyze and apply call is self-contained
- **Bring your own inference**: any service that reproduces the wire contract is a drop-in replacement for the shipped bento services, including self-hosted or custom models and weights

## Quick Start

The fastest way to get started is with [Nvisy Cloud](https://nvisy.com).

For self-hosted use, embed the engine crate directly and deploy the
inference services from [nvisycom/bento](https://github.com/nvisycom/bento)
as sidecar containers. See each crate README for details.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
