# nvisy-schema

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Wire schema for the Nvisy platform: request and response body types,
policy documents, analyzer plans, and file metadata.

## Overview

The SDK-safe subset of Nvisy's type surface. Ships the serde-derived
types that appear on the wire and the JSON schema derivations for them.

Sibling to nvisy-core, which adds the deployment-side runtime
configuration (NER and LLM recognizer lineups, error vocabulary) hosts
need to construct an engine. SDK consumers who only need to
(de)serialize wire bodies should depend on nvisy-schema directly to
avoid pulling those extras. The transitive dependency tree is
elide-core plus a slice of the serialization stack, no HTTP client, no
LLM plumbing.

## Changelog

See ../../CHANGELOG.md for release notes and version history.

## License

Apache 2.0 License, see ../../LICENSE.txt.

## Support

- Documentation: <https://docs.nvisy.com>
- Issues: <https://github.com/nvisycom/runtime/issues>
- Email: support@nvisy.com
