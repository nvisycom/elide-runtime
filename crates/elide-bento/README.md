# elide-bento

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/rs-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/rs-build.yml)

Shared BentoML HTTP client wrapper for elide backends.

## Overview

Per-modality backends (NER, OCR, …) live in their consuming crates
(`elide-ner`, `elide-ocr`) and pull this crate for the common HTTP
client, params validation, and error translation. The Python services
this client talks to live under [`packages/`](../../packages) in the
same workspace.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
