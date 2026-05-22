# nvisy-ontology

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Domain data types for the Nvisy platform — entities, locations,
artifacts, and the redaction/review provenance shared across every
crate in the workspace.

## Overview

`entity::*` defines the hierarchical entity taxonomy (`EntityKind`,
`EntityCategory`, `Confidence`, `RecognitionMethod`, etc.) and
`Entity`/`Entities` collection types used as the output of every
recognizer.

`primitive::*` carries cross-cutting value types: `BoundingBox`,
`LanguageTag`, time spans, etc.

`policy::*` describes how a detected entity should be redacted —
text strategies (`Mask`, `Replace`, `Hash`, `Encrypt`), image
strategies (`Blur`, `Pixelate`, `Block`), and the matching rules
that pick a strategy per entity.

`provenance::*` records what the pipeline did: `RedactionMap`,
`AuditEntry`, `ReviewStatus`, and the redaction-value index for
reversibility.

`artifacts::*` and `context::*` carry the per-document state
threaded through the engine (extracted text/image artifacts,
analyser hints).

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
