# nvisy-schema

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/rs-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/rs-build.yml)

Wire schema for the Nvisy platform: umbrella crate re-exporting policy
and context alongside plan and file types.

## Overview

The SDK-safe subset of Nvisy's type surface. Ships the serde-derived
types that appear on the wire and the JSON schema derivations for them.

Structured as an umbrella. `plan`, `file`, and the `elide-core` slice
(`primitive`, `entity`, `modality`) live in this crate directly;
`policy` and `context` come from their peer crates, re-exported so a
single `nvisy-schema` dep still gives an SDK caller the whole wire
surface. Consumers who want tighter dep trees can depend on
`nvisy-policy` or `nvisy-context` directly and skip this crate.

Sibling to `nvisy-core`, which adds the deployment-side runtime
configuration (NER and LLM recognizer lineups, error vocabulary) hosts
need to construct an engine. The transitive dependency tree for
`nvisy-schema` is `elide-core` plus a slice of the serialization stack:
no HTTP client, no LLM plumbing.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
