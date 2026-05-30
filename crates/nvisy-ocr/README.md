# nvisy-ocr

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

OCR backend abstraction for the Nvisy runtime. Defines the
`Backend` trait and an `OcrBackend` config enum that selects between
built-in implementations behind a uniform `Extractor` surface.

## Overview

`Extractor` is the dispatch entry point — built from an
`OcrBackend` enum variant via `into_extractor()`, then called as
`extractor.extract(image, ctx)`. Each backend implements the
crate's `Backend` trait and produces unified `ImageOutput` (text +
line/word geometry).

Two backends ship today:

- **`NoopBackend`** — produces zero OCR results. The default;
  used in tests and in deployments that accept image content but
  don't OCR it.
- **`BentoBackend`** (feature `bento`) — scaffolding for the
  externalised `inference-ocr` Bento in [`nvisycom/inference`].
  Not yet functional; tracked under [#128].

VLM-mediated entity verification (the LLM-side counterpart that
verifies image-side entity proposals against the source image)
lives in `nvisy-agent` as `VlmVerifyAgent`.

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
