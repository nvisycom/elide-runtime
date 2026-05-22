# Nvisy Runtime

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Multimodal redaction runtime for sensitive data.

Detect and remove sensitive information across documents, images, and audio.
Combines deterministic patterns, NER, computer vision, and LLM-driven
classification into auditable, policy-driven pipelines built for regulated
industries such as healthcare, legal, government, and financial services.

## Features

- **Multimodal codecs**: read, edit, and write PDF, DOCX, images, audio, CSV, JSON, and plain text through a unified span-based content model
- **Layered detection**: regex, dictionary, and checksum patterns run first at low cost; NER, OCR, object detection, and LLM classification handle what deterministic methods cannot
- **Context-aware redaction**: mask, replace, hash, encrypt, blur, block, and pixelate with policy-driven rules scoped to entity type, document class, and confidence threshold
- **Pipeline engine**: flat fixed-order pipeline (extraction → detection → dedup → redaction → validation) with concurrent per-document execution
- **Pure Rust**: no embedded Python; OCR/STT/NER reach external providers through a shared HTTP client

## Quick Start

The fastest way to get started is with [Nvisy Cloud](https://nvisy.com).

For self-hosted deployments, refer to [`docker/`](docker/) for compose files and
infrastructure requirements, and [`.env.example`](.env.example) for configuration.

## Documentation

See [`docs/`](docs/) for architecture, security, and API documentation.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
