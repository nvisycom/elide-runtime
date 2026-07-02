# nvisy-core

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/py-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/py-build.yml)

Shared wire-contract types for the Nvisy inference services. The OCR, NER,
and vision-language OCR services all depend on this package, so the HTTP
contract is defined once on the Python side.

## Overview

Versioned pydantic models describe each service's request and response shapes.
Import a specific version explicitly:

- [`nvisy_core.ocr.v1`](src/nvisy_core/ocr/v1.py) — OCR contract
  (`Page → Block → Line → Word`, geometry as axis-aligned `BoundingBox` plus
  optional polygon).
- [`nvisy_core.ocrvl.v1`](src/nvisy_core/ocrvl/v1.py) — vision-language OCR
  contract (block-level regions with text, layout kind, bbox, and reading
  order).
- [`nvisy_core.ner.v1`](src/nvisy_core/ner/v1) — NER contract (`Entity` with
  label, score, and character offsets).

The wire is camelCase, mirroring the Rust side's serde
`rename_all = "camelCase"`. These pydantic models are the source of truth
for the wire contract; the Rust [`elide-bento`](../../crates/elide-bento)
client mirrors them by hand.

## Documentation

See [`docs/`](../../docs/) for per-service rationale and design notes.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
