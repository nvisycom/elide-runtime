# nvisy-ocr

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

OCR `Backend` trait and dispatch `Extractor` for the Nvisy runtime.

## Overview

`Extractor` wraps any `Backend` implementation as an
`Extractor<Image>` consumed by the document orchestrator; each
backend produces unified `OcrOutput` (text + line/word geometry).
Shipped backends: `NoopBackend` (default, returns no results — used
in tests and OCR-less deployments) and the feature-gated
`BentoBackend`, an HTTP client for the externalised `inference-ocr`
Bento in [`nvisycom/inference`] (scaffolding only; tracked under
[#128]).

VLM-mediated entity verification (the LLM-side counterpart that
checks image-side entity proposals against the source image) lives
in `nvisy-llm`, not here.

[`nvisycom/inference`]: https://github.com/nvisycom/inference
[#128]: https://github.com/nvisycom/runtime/issues/128

## Documentation

See [`docs/`](../../docs/) for architecture, security, and API documentation.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
