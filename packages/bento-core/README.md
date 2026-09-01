# bento-core

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-provider/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-provider/actions/workflows/build.yml)

Shared wire-contract types for the elide-provider inference services. The OCR,
NER, vision-language OCR and speech-to-text services all depend on this
package, so the HTTP contract is defined once on the Python side.

## Overview

Versioned pydantic models describe each service's request and response shapes.
Import a specific version explicitly:

- [`bento_core.ocr.v1`](src/bento_core/ocr/v1.py) — OCR contract
  (`Page → Block → Line → Word`, geometry as axis-aligned `BoundingBox` plus
  optional polygon).
- [`bento_core.ocrvl.v1`](src/bento_core/ocrvl/v1.py) — vision-language OCR
  contract (block-level regions with text, layout kind, bbox, and reading
  order).
- [`bento_core.ner.v1`](src/bento_core/ner/v1) — NER contract (`Entity` with
  label, score, and character offsets).
- [`bento_core.stt.v1`](src/bento_core/stt/v1.py) — speech-to-text
  contract (base64 audio in; segments with millisecond timings, optional
  speaker label, language, confidence, and per-word breakdown out).

The wire is camelCase, mirroring the Rust side's serde
`rename_all = "camelCase"`. These pydantic models are the source of truth
for the wire contract; the Rust [`elide-bentoml`](../../crates/elide-bentoml)
client mirrors them by hand.

## License

Apache 2.0 License, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-provider/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
