# nvisy-ocr

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

OCR backend trait, type-erased engine, and provider implementations for the Nvisy runtime.

Defines the [`Backend`] trait for text extraction from images and ships six
provider implementations across local and cloud services:

**Local** (always available):
- [`DoctrBackend`]: DocTR server (multipart upload, normalised coordinates)
- [`PaddleXBackend`]: PaddleX PP-OCRv5 server (multipart upload, word-level boxes)
- [`SuryaBackend`]: Surya OCR server (multipart upload, pixel coordinates)

**Cloud** (feature-gated):
- [`AwsTextractBackend`]: AWS Textract with inline SigV4 signing (`aws` feature)
- [`GoogleVisionBackend`]: Google Cloud Vision API (`google` feature)
- [`AzureDocaiBackend`]: Azure Document Intelligence with async polling (`azure` feature)

Every backend returns [`ImageOutput`] containing a list of [`ImageRegion`]s,
each with extracted text, optional confidence score, bounding box, polygon
vertices for rotated text, and hierarchical text-level annotations.

The [`Engine`] wrapper provides a type-erased entry point with built-in
`tracing` instrumentation for request-level observability.

## Feature flags

| Flag | Enables |
|----------|----------------------------------------------|
| `aws` | AWS Textract provider |
| `google` | Google Cloud Vision provider |
| `azure` | Azure Document Intelligence provider |

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
