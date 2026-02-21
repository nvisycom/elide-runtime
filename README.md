# Nvisy Runtime

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Open-source multimodal redaction runtime. Detect, redact, and audit PII and
sensitive data across documents, images, audio, and video.

## Features

- **Multimodal Codecs:** Read, edit, and write PDF, DOCX, images, audio, CSV, JSON, and plain text
- **AI-Powered Detection:** Regex, dictionary, checksum, NER, and LLM-driven entity recognition
- **Span-Aware Redaction:** Mask, replace, hash, encrypt, blur, block, pixelate, and synthesize
- **Pipeline Engine:** DAG compiler and executor with retry and timeout policies
- **Python Extensions:** PyO3 bridge for AI-powered NER and OCR via embedded CPython

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
- **API Status**: [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
