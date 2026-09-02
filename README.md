<div align="center">

# Elide Runtime

**Multimodal redaction pipeline as a stateless Rust library.**

Governance vocabulary and ready-to-run regulatory policy templates, over the
Elide detection and redaction toolkit.

[![Runtime](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=runtime&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)
[![Inference](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-provider/build.yml?branch=main&label=inference&style=flat-square)](https://github.com/nvisycom/elide-provider/actions/workflows/build.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](LICENSE.txt)

[**nvisy.com**](https://nvisy.com) · [**docs.nvisy.com**](https://docs.nvisy.com)

</div>

Elide Runtime layers on top of the [Elide](https://github.com/nvisycom/elide)
toolkit. Elide provides the low-level primitives (recognizers, anonymizers,
and the tamper-evident audit log) and Elide Runtime wires them into a
document-oriented pipeline. Use this crate set alongside Elide, not instead
of it.

A workspace of library crates a host embeds directly. No long-running process,
no HTTP layer of its own. Inference is delegated over HTTP to model services
in the sibling
[nvisycom/elide-provider](https://github.com/nvisycom/elide-provider)
repository; the engine ships with a client that speaks their wire contract,
and any service reproducing that contract is a drop-in replacement.

> [!WARNING]
> **Active development: API not stable.** This project is under active
> development. Public APIs, configuration shapes, and wire schemas may
> change without notice between releases. Pin a specific commit if you
> depend on this in production.

## Features

**Multimodal codecs**  
Read, edit, and write PDF, DOCX, images, audio, CSV, JSON, and plain text through a unified span-based content model.

**Layered detection**  
Regex, dictionary, and checksum patterns run first at low cost; NER, OCR, VLM, and LLM classification handle what deterministic methods cannot.

**Context-aware redaction**  
Mask, replace, hash, encrypt, blur, block, and pixelate, with policy-driven rules scoped to entity type, document class, and confidence threshold.

**Regulatory templates**  
Ready-to-run policy postures for HIPAA, GDPR, PCI DSS, CCPA, and SOC 2, so a common obligation does not start from an empty rule set.

**Stateless engine**  
No persistence, no HTTP layer, no background tasks. Each call to `analyze` or `anonymize` is self-contained.

**Bring your own inference**  
Any service reproducing the wire contract is a drop-in replacement for the shipped services, including self-hosted or custom models and weights.

## Quick Start

The fastest way to get started is with [Nvisy Cloud](https://nvisy.com).

For self-hosted use, embed the engine crate directly and deploy the inference
services from [nvisycom/elide-provider](https://github.com/nvisycom/elide-provider)
as sidecar containers. See each crate README for details.

## Project

- **Changelog**: [CHANGELOG.md](CHANGELOG.md) for release notes and version history
- **Contributing**: [CONTRIBUTING.md](CONTRIBUTING.md)
- **License**: Apache 2.0, see [LICENSE.txt](LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
