# nvisy-document

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Whole-document carrier, policy store, and per-document phase
orchestrator that drives a document through extraction → detection
→ deduplication → redaction → validation in the Nvisy runtime.

## Overview

Owns the typed `Document<M>` carrier (with per-modality `Block<M>`,
`Span<M>`, audit, metadata) and the `DocumentTree<M>` shape the
orchestrator walks. Nested documents — PDF page images, embedded
figures — sit inside their parent text flow via the
`TextBlock::Embed` variant without losing per-modality typing.

Hosts the per-run plumbing (`SharedData`, `RunContext`,
`PolicyStore`), the TOML-deserialisable phase configs that consumers
drop into `Nvisy.toml`, and the closed selector enums (`NerBackend`,
`OcrBackend`, `SttBackend`) that pick a concrete backend at boot.
Pulls `nvisy-core` for the atoms, `nvisy-codec` for ingest, and
`nvisy-toolkit` for the components each phase calls into.

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
