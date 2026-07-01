# nvisy-schema

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Wire schema for the Nvisy HTTP API: request and response body types, policy
documents, analyzer plans, and file metadata.

## Overview

This crate is the SDK-safe subset of Nvisy's type surface. It ships the
serde-derived types that appear on the wire (`Policy`, `AnalyzerParams`,
`FileMetadata`, `Context`, …) and the JSON schema derivations for them.

The heavier `nvisy-core` crate depends on this one and adds the deployment
configuration types (LLM provider clients, healthcheck traits) that only
the server needs. SDK consumers should depend on `nvisy-schema` directly
to avoid pulling those extras — the transitive dep tree is `elide-core`
+ a slice of the serialization stack, no HTTP client, no LLM plumbing.

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
