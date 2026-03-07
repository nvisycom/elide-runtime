# nvisy-ontology

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Domain data types for the Nvisy platform. Provides entity detection outputs, modality-specific location types, redaction specifications, policy review types, and annotation structures used across the detection and redaction pipeline.

- **Entity taxonomy**: hierarchical entity types (PII, PHI, PCI, etc.) with confidence scores
- **Detection outputs**: unified detection results spanning text, image, and audio modalities
- **Modality locations**: text spans, bounding boxes, and temporal ranges for locating entities
- **Redaction operations**: mask, replace, hash, encrypt, blur, block, and pixelate specifications
- **Review annotations**: human-in-the-loop review decisions, approval states, and audit trails

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
