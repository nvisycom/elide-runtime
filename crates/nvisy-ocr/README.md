# nvisy-ocr

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

OCR backend abstraction for the Nvisy runtime. Defines the
`Backend` trait and an `OcrBackend` config enum that selects between
built-in implementations behind a uniform `OcrEngine` surface.

## Overview

`OcrEngine` is the dispatch entry point — built from an
`OcrBackend` enum variant via `into_engine()`, then called as
`engine.run(input, params)`. Each backend implements the crate's
`Backend` trait and produces unified `ImageOutput` (text +
line/word geometry).

Two backends ship today:

- **`NoopOcrBackend`** — produces zero OCR results. The default;
  used in tests and in deployments that accept image content but
  don't OCR it.
- **`BentoOcrBackend`** (feature `bento`) — scaffolding for the
  externalised `inference-ocr` Bento in [`nvisycom/inference`].
  Not yet functional; tracked under [#128].

Cloud backends (AWS Textract, Google Cloud Vision, Azure Document
Intelligence) lived here previously and have been removed to clear
the deck for the externalised architecture. Reintroduction is
tracked under [#201] / [#202] / [#203].

LLM-mediated entity verification (the LLM-side counterpart that
verifies OCR-proposed entities against the source image) lives in
`nvisy-agent` as `CvVerifyAgent`.

[`nvisycom/inference`]: https://github.com/nvisycom/inference
[#128]: https://github.com/nvisycom/runtime/issues/128
[#201]: https://github.com/nvisycom/runtime/issues/201
[#202]: https://github.com/nvisycom/runtime/issues/202
[#203]: https://github.com/nvisycom/runtime/issues/203

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
