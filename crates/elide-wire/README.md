# elide-wire

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Wire schemas for the `elide-runtime` pipeline: plan and file, plus a
re-export of `elide-governance`.

## Overview

Layers on top of the [elide](https://github.com/nvisycom/elide) toolkit.
The SDK-safe subset of the platform's type surface: the serde-derived
types that appear on the wire and their JSON schema derivations.

`plan` (analyzer parameters) and `file` (document envelope) live in
this crate directly; `policy` comes from its peer crate
(`elide-governance`), re-exported so a single `elide-wire` dep gives
an SDK caller the whole wire surface. Primitives, entities, modalities,
and annotations stay in `elide-core` — depend on it directly.

Consumed by `elide-pipeline`, which layers the deployment-side runtime
configuration (NER and LLM recognizer lineups) hosts need to construct
an engine. The transitive dependency tree for `elide-wire` is
`elide-core` plus a slice of the serialization stack: no HTTP client,
no LLM plumbing.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
