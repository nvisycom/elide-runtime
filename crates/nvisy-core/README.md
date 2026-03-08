# nvisy-core

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Foundational crate for the Nvisy runtime. Defines domain types, error types, the plugin trait system, and the action/provider registry that all other crates build on.

- **Error types**: structured error hierarchy for pipeline, codec, detection, and provider failures
- **Plugin trait system**: `Plugin`, `Action`, and `Provider` traits with a capability registry
- **Content model**: span-based representation of document content across modalities
- **File abstraction**: unified `File` type with MIME detection and byte-level access
- **Math primitives**: bounding boxes, polygons, and geometric operations for spatial detection

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
