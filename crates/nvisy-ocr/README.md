# nvisy-ocr

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

OCR backend trait and provider integration for the Nvisy runtime.

Defines the [`OcrBackend`] trait for text extraction from images and provides
two implementations:

- **local:** Rust-native PaddleOCR via ONNX Runtime (oar-ocr)
- **bridge:** Python-based OCR engines via the PyO3 bridge

Each backend returns typed [`OcrRegion`] results with bounding boxes, optional
polygon vertices for rotated text, and hierarchical text-level annotations.

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
