# elide-pipeline

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Stateless multimodal redaction pipeline over elide.

## Overview

The engine bundles Elide's codec, detection, recognition, redaction,
and orchestration layers into a single per-request pipeline. A caller
builds a Provider from a ProviderConfig, wraps it with Engine::new,
then drives documents through the analyze and apply verbs. Every call is
self-contained: no persistence, no HTTP layer, no long-running
background tasks. Hosts (a SaaS backend, a Tauri app, a CLI, an SDK)
own their own workflow and storage on top.

Owns the two verbs and the document model between them: analyze
returns an audit a host holds and edits however it sees fit, and
anonymize applies it. Building the orchestrators those verbs run on
belongs to elide-provider, which turns a deployment's configuration
into Elide runtime values; this crate holds an Engine over one and
knows nothing about where that configuration came from.

This crate is the umbrella entry point: everything a caller needs is
surfaced here, including the provider vocabulary, the request schemas
(plan and file), the underlying Elide toolkit, and the governance
vocabulary. A host depends on this crate alone.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
