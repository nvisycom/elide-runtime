# nvisy-registry

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Actor-scoped content and context storage backed by fjall. Provides typed identifiers (`ActorId`, `ContentId`, `ContextId`) and a unified `Registry` for managing content files and detection contexts with actor-level isolation.

- **fjall storage**: embedded LSM-tree key-value store for persistent content and context data
- **Actor isolation**: each actor operates in its own namespace preventing cross-tenant data access
- **Typed identifiers**: `ActorId`, `ContentId`, and `ContextId` enforce type-safe lookups at compile time

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
