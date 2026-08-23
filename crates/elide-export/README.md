# elide-export

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Export formats for Elide Runtime audits: JSON documents and CSV
tables.

## Overview

Two formats, two traits, because they answer different questions.
ExportJson writes one document whole: every field, every entity's
provenance chain, nested as it is in memory. It is blanket
implemented for any serializable type, so a type gains JSON export by
deriving serialization.

ExportCsv writes tables. CSV cannot nest, so one audit becomes
several flat relations that join on a shared key. A caller writes one
table by name, or iterates the declared set to write them all; with
the zip feature they can be bundled into a single archive.

Keeping the two apart matters because the shapes differ, not just the
syntax. A JSON export is lossless and mechanical. A CSV export is a
deliberate projection: which columns survive flattening, and how a
polymorphic location or a nested payload renders as a scalar. Every
such choice is loss, so it belongs in a named table with documented
columns rather than hidden inside one writer.

Each format is self-contained behind its own feature, and neither
pulls the engine in: this crate depends on elide-core for the shared
error type and nothing else.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
