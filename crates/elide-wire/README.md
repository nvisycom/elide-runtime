# elide-wire

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Wire schemas for the Elide Runtime pipeline: plan (analyzer
parameters) and file (document envelope).

## Overview

Layers on top of the [Elide](https://github.com/nvisycom/elide) toolkit.
The SDK-safe subset of the platform's surface: everything that appears
on the wire, plus the JSON schemas derived from it.

Plan (analyzer parameters) and file (document envelope) live in this
crate directly. Governance documents (policies, rules, predicates,
operators) live in the peer crate elide-governance; consumers depend
on it directly rather than through this crate. Primitives, entities,
modalities, and annotations stay in elide-core, which consumers also
depend on directly.

Consumed by elide-pipeline, the umbrella entry-point crate. The
transitive dependency tree here is elide-core plus a slice of the
serialization stack: no HTTP client, no LLM plumbing.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
