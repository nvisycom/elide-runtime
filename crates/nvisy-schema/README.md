# nvisy-schema

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Wire schema for the Nvisy platform: plan, file, and the `elide-core`
slice, plus a re-export of `nvisy-policy`.

## Overview

The SDK-safe subset of Nvisy's type surface. Ships the serde-derived
types that appear on the wire and the JSON schema derivations for them.

`plan`, `file`, and the `elide-core` slice (`primitive`, `entity`,
`modality`) live in this crate directly; `policy` comes from its peer
crate, re-exported so a single `nvisy-schema` dep gives an SDK caller
the whole wire surface. Consumers who want tighter dep trees can
depend on `nvisy-policy` directly and skip this crate.

Consumed by `nvisy-engine`, which layers the deployment-side runtime
configuration (NER and LLM recognizer lineups) hosts need to construct
an engine. The transitive dependency tree for `nvisy-schema` is
`elide-core` plus a slice of the serialization stack: no HTTP client,
no LLM plumbing.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
