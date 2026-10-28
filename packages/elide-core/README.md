# elide-core

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Shared wire-contract types for this runtime's inference services. The NER,
OCR and speech-to-text services all depend on this package, so the HTTP
contract is defined once on the Python side.

## Overview

Versioned pydantic models describe each service's request and response shapes.
Import a specific version explicitly:

- [`elide_core.ocr.v1`](src/elide_core/ocr/v1.py) — OCR contract
  (`Page → Block → Line → Word`, geometry as axis-aligned `BoundingBox` plus
  optional polygon).
- [`elide_core.ner.v1`](src/elide_core/ner/v1) — NER contract (`Entity` with
  label, score, and character offsets).
- [`elide_core.stt.v1`](src/elide_core/stt/v1.py) — speech-to-text
  contract (base64 audio in; segments with millisecond timings, optional
  speaker label, language, confidence, and per-word breakdown out).

Wire field names use snake_case in Python, JSON, and the generated Rust types.

## Versioning

Versioned per contract module (`ner.v1`, `ocr.v1`, `stt.v1`) rather than per
release: a breaking wire change lands as `v2` alongside `v1`, so a service and
its client can migrate independently.

Python models are authoritative for NER, OCR, and STT. JSON Schema, shared
fixtures, and Rust wire types are generated together; see [wire contracts](../../contracts).
Rust domain types and their conversions remain handwritten. Regenerate all
services with `uv run python scripts/gen_contracts.py`; CI checks for drift.

## Project

- **Changelog**: [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history
- **Contributing**: [CONTRIBUTING.md](../../CONTRIBUTING.md) for setup, local CI targets, and the pull-request process
- **License**: Apache 2.0, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
