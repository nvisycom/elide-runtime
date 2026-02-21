# nvisy-detection

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Entity ontology types, detection layers, and pattern/dictionary infrastructure for the Nvisy runtime.

Defines the core entity model (`Entity`, `DetectionMethod`, locations), detection traits (`DetectionLayer`, `Detect`), and concrete detection layers for text (regex patterns, Aho-Corasick dictionaries, NER), tabular data (column rules), and documents (checksum validation, manual annotations).

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
