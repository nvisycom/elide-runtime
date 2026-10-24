<div align="center">

<img src=".github/assets/logo.png" alt="Elide Runtime" width="104" height="104" />

# Elide Runtime

**Multimodal redaction pipeline as a stateless Rust library.**

Governance vocabulary and ready-to-run regulatory policy templates over the
Elide detection and redaction toolkit, with the inference services behind them.

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)
[![Security](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/security.yml?branch=main&label=security&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/security.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue?style=flat-square)](LICENSE)

[**nvisy.com**](https://nvisy.com) · [**docs.nvisy.com**](https://docs.nvisy.com)

</div>

Elide Runtime layers on top of the [Elide](https://github.com/nvisycom/elide)
toolkit. Elide provides the low-level primitives (recognizers, anonymizers,
and the tamper-evident audit log) and Elide Runtime wires them into a
document-oriented pipeline. Use this crate set alongside Elide, not instead
of it.

The crates in [`crates/`](crates) are libraries a host embeds directly: no
long-running process, no HTTP layer of their own. Inference is delegated over
HTTP to the model services in [`packages/`](packages), each shipped as a
Docker container and deployed as a sidecar. The
[adapters](crates/adapter) speak their wire contract, and any service
reproducing that contract is a drop-in replacement.

> [!WARNING]
> **Active development: API not stable.** This project is under active
> development. Public APIs, configuration shapes, and wire schemas may
> change without notice between releases. Pin a specific commit if you
> depend on this in production.

## Features

**Multimodal codecs**  
Read, edit, and write PDF, Office documents (DOCX, XLSX, PPTX), images, audio, CSV, JSON, HTML, XML, and plain text through four typed modalities: text, image, audio, and tabular.

**Layered detection**  
Regex, dictionary, and checksum patterns run first at low cost; NER, OCR, VLM, and LLM classification handle what deterministic methods cannot.

**Context-aware redaction**  
Mask, replace, hash, encrypt, blur, block, and pixelate, with policy-driven rules scoped to entity type, document class, and confidence threshold.

**Regulatory templates**  
Ready-to-run policy postures for HIPAA, GDPR, PCI DSS and CCPA, so a common obligation does not start from an empty rule set.

**Stateless engine**  
No persistence, no HTTP layer, no background tasks. Each call to `analyze` or `anonymize` is self-contained.

**Bring your own inference**  
Any service reproducing the wire contract is a drop-in replacement for the shipped services, including self-hosted or custom models and weights.

## Engine

**[elide-pipeline](crates/elide-pipeline)**  
The entry point: `analyze` and `anonymize` over one document, and the audit that carries the handoff between them.

**[elide-deployment](crates/elide-deployment)**  
What an operator decides once: which backends exist, how to reach them, and which of them a caller may select. Loadable from TOML.

**[elide-provider](crates/elide-provider)**  
Turns that configuration into the Elide values a request needs: recognizers, enrichers, anonymizers.

**[elide-schema](crates/elide-schema)**  
The request-side wire vocabulary: policies, scopes, rules, predicates, redactions.

**[elide-template](crates/elide-template)**  
The regulatory postures, each a single policy a caller hands to the engine.

**[elide-review](crates/elide-review)**  
Reviewer decisions over detected entities: add, retag, suppress.

**[elide-export](crates/elide-export)**  
Rendering an audit into JSON or flat CSV tables.

## Services

**[elide-ner](packages/elide-ner)**  
Schema-driven NER and PII detection, wrapping GLiNER2, so the labels to find are named per call rather than baked into the model.

**[elide-ocr](packages/elide-ocr)**  
Document OCR with word-level geometry, wrapping docTR.

**[elide-stt](packages/elide-stt)**  
Speech-to-text, so audio reaches the same detection pipeline as text.

Each package is named for the modality it serves, not the model behind it: the
model is an env-selected backend inside the service, so a successor ships as a
module rather than a new package.

## Adapters

**[elide-bentoml](crates/adapter/elide-bentoml)**  
The client for the services above, feature-gated per backend (`ner`, `ocr`, `stt`).

**[elide-aws-textract](crates/adapter/elide-aws-textract)**  
An AWS Textract-backed OCR backend, for hosted document text detection instead
of self-hosted docTR.

**[elide-deepgram](crates/adapter/elide-deepgram)**  
A Deepgram-backed speech-to-text backend, for hosted transcription instead of
self-hosted Whisper.

**[elide-google-document-ai](crates/adapter/elide-google-document-ai)**  
A Google Cloud Document AI-backed OCR backend, the other hosted alternative to
`elide-ocr`.

**[elide-pioneer-gliner](crates/adapter/elide-pioneer-gliner)**  
A Pioneer-backed NER backend, running the same GLiNER2 model family as
`elide-ner` without self-hosting it.

## Contract

**[elide-core](packages/elide-core)**  
The wire-contract types the services share. Not a service itself: no
`bentofile.yaml`, no serve target, just the shapes both sides agree on.

Each adapter speaks its service through that contract, not the model behind
it. Any HTTP service reproducing the `/recognize` (NER, OCR) or
`/transcribe` (STT) contract is a drop-in replacement for the shipped
packages, including self-hosted or custom models and weights. Each package
README documents its wire shape.

## Quick Start

The fastest way to get started is with [Nvisy Cloud](https://nvisy.com).

For self-hosted use, embed the engine crates directly and run the inference
services as sidecar containers:

```bash
make sync             # install the Python workspace dependencies
make serve-ner        # or serve-ocr, serve-stt
```

or build the Docker images:

```bash
make services         # build every Bento
make services-image   # build and containerize
```

See each crate and package README for details.

## Project

- **Changelog**: [CHANGELOG.md](CHANGELOG.md) for release notes and version history
- **Contributing**: [CONTRIBUTING.md](CONTRIBUTING.md) for setup, local CI targets, and the pull-request process
- **License**: Apache 2.0, see [LICENSE](LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
