# nvisy-ocr

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

OCR provider integrations for the Nvisy runtime. Wraps third-party
OCR services behind a uniform `OcrEngine` / `Backend` surface.

## Overview

`OcrEngine` is the dispatch entry point — construct one from an
`OcrProvider` enum variant and call `run(input, params)`. Each
backend implements the crate-internal `Backend` trait, accepts an
`HttpClient` from the caller (no global state), and produces
unified `ImageOutput` (text + line/word geometry) regardless of
provider.

Built-in backends (default features):

- **Surya** (`SuryaBackend`) — Datalab Surya HTTP backend.
- **PaddleX** (`PaddleXBackend`) — Baidu PaddleX HTTP backend.

Feature-gated backends:

- **AWS Textract** (`aws-textract` feature) — `AwsTextractBackend`.
- **Google Cloud Vision** (`google-vision` feature) — `GoogleVisionBackend`.
- **Azure Document Intelligence** (`azure-docai` feature) — `AzureDocaiBackend`.

LLM-mediated entity verification (the LLM-side counterpart that
verifies OCR-proposed entities against the source image) lives in
`nvisy-agent` as `CvVerifyAgent`. HTTP transport (`HttpClient`,
`HttpConfig`, retry + tracing middleware) lives in the shared
`nvisy-http` crate.

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
