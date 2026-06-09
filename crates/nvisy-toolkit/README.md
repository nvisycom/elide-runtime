# nvisy-toolkit

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Composable component library for Nvisy pipelines — the registries
and policies a consumer plugs into their own document-processing
flow.

## Overview

Hosts the per-stage component machinery that the document
orchestrator drives: `RecognizerRegistry` + `Detection` plan (which
recognizers run per modality), `ExtractorRegistry` + `Extraction`
plan (OCR / STT), `LayerPipeline` for post-detection deduplication,
`CheckPipeline` for validation, and `RedactionRegistry<M>` for
custom anonymizers.

Also ships the `MemoryBuffer<M>` ingestion helper that wraps bytes
in a `DataAt<M>` (and `RedactAt<Text>`) buffer suitable for direct
consumption by recognizers. Sits one level above `nvisy-core`:
toolkit owns reusable pieces; the orchestration that strings them
into a full pipeline lives one layer up.

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
