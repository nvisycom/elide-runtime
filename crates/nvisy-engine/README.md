# nvisy-engine

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Stateless multimodal redaction pipeline over elide.

## Overview

The engine bundles elide's codec, detection, recognition, redaction,
and orchestration layers into a single per-request pipeline. Callers
construct an engine paired with the deployment's NER and LLM lineups
(from nvisy-core), then drive documents through analyze and apply
verbs. Every call is self-contained: no persistence, no HTTP layer, no
long-running background tasks. Hosts (a SaaS backend, a Tauri app, a
CLI, an SDK) own their own workflow and storage on top.

Owns the per-request orchestrator constructor that wires elide's
per-modality Analyzer and Anonymizer against a request-scoped Scope,
with policies and reviewer overrides layered onto each modality's
anonymizer. Recognizers come from nvisy-schema-shaped analyzer
parameters plus the deployment's NER and LLM configurations. The
analyze verb returns a modality-tagged document body that hosts hold
between analyze and apply, however they see fit.

## Changelog

See ../../CHANGELOG.md for release notes and version history.

## License

Apache 2.0 License, see ../../LICENSE.txt.

## Support

- Documentation: <https://docs.nvisy.com>
- Issues: <https://github.com/nvisycom/runtime/issues>
- Email: support@nvisy.com
