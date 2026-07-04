# nvisy-context

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/rs-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/rs-build.yml)

Wire schema for Nvisy contexts: persistent reference-data collections
that tell detection *what to look for*.

## Overview

A `Context` holds reusable reference data (names, faces, voices,
patterns, embeddings) that recognizers consult during detection. It is
separate from policy, which controls *what to do* when something is
found.

Sibling to `nvisy-policy` (what to do when detection fires) and
`nvisy-schema` (umbrella re-exporting both).

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
