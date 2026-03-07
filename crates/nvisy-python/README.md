# nvisy-python

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Thin PyO3 bridge between Rust and Python. Manages interpreter lifecycle, defines intermediate types for cross-boundary data exchange, and provides a calling convention for Python packages under [`packages/`](../../packages/).

- **Interpreter management**: embeds and initialises the Python runtime via PyO3
- **Intermediate types**: Rust structs that map to Python objects for passing data across the FFI boundary
- **Package loading**: discovers and imports Python packages at runtime
- **Calling convention**: uniform interface for invoking any Python package from Rust engine actions

Individual capabilities (NER, EXIF, etc.) live in their own Python packages; this crate only provides the bridge.

Requires Python >= 3.11 at build time and runtime.

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
