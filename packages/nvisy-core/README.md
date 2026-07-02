# nvisy-core

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/py-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/py-build.yml)

Shared wire-contract types for the Nvisy inference services. The OCR and NER
services both depend on this package, so the HTTP contract is defined once on
the Python side.

## Overview

Versioned pydantic models describe each service's request and response shapes.
Import a specific version explicitly:

- `nvisy_core.ocr.v1` — OCR contract (`Page → Block → Line → Word`, geometry as
  axis-aligned `BoundingBox` plus optional polygon).
- `nvisy_core.ner.v1` — NER contract (`Entity` with label, score, and character
  offsets).

The wire is camelCase, mirroring the runtime's serde `rename_all = "camelCase"`.
The Rust runtime (`nvisy-inference-client`) is the ultimate source of truth;
these models mirror it. Machine-readable OpenAPI specs are generated from the
services into [`docs/openapi/`](../../docs/openapi) and are the contract for
bring-your-own-inference implementers.

## Documentation

See [`docs/`](../../docs/) for the generated OpenAPI specs and contract
documentation.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/inference/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
