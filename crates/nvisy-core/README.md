# nvisy-core

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Deployment-side runtime plumbing for the Nvisy platform: NER and LLM
recognizer lineups plus the shared error vocabulary every runtime crate
returns from.

## Overview

Sibling to nvisy-schema (the wire schema). This crate holds the
deployment-only surface an SDK caller doesn't need. The LLM module
carries the deployment-owned recognizer lineup with provider
credentials, wrapping elide-llm's provider types; the wire only
toggles LLM on or off, the deployment picks which providers actually
run. The NER module mirrors that shape for NER backends. The error
vocabulary is distinct from elide's own, adding request-scoped
context and surface categories the toolkit doesn't model.

Consumed by nvisy-engine to construct an engine. SDK consumers who
only need to (de)serialize wire bodies should depend on nvisy-schema
directly.

## Changelog

See ../../CHANGELOG.md for release notes and version history.

## License

Apache 2.0 License, see ../../LICENSE.txt.

## Support

- Documentation: <https://docs.nvisy.com>
- Issues: <https://github.com/nvisycom/runtime/issues>
- Email: support@nvisy.com
