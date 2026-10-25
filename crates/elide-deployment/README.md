# elide-deployment

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

What a deployment decides once, before any request arrives.

## Overview

A Deployment is the whole of an operator's configuration: which
backends exist and how to reach them, which of them a caller may
select, and how language detection is narrowed. A host loads one at
startup and hands it to elide-provider, which turns it into the
backends it names.

This is deliberately not request schema. Everything here belongs to
whoever operates the deployment: it holds API keys and base URLs, and
a request never sends any of it. What a request may say lives in
elide-schema — the policies and the vocabulary — and in
elide-provider's RequestContext. Keeping the two apart is what lets
the request surface stay safe to serialize, log and export, while the
deployment surface is treated as a secret.

A deployment is read from a TOML file, from a document already in
hand, or built in code; all three produce the same value and run the
same checks. Secrets are named rather than held: a `${VAR}` reference
is expanded from the process environment at load, so the file itself
can live wherever the configuration does. A reference that cannot be
resolved fails the load — never an empty string, because an empty
credential would reach the backend as a credential and surface as an
authentication error far from its cause.

## Validated by construction

A Deployment that exists is one the runtime can build. The fields are
read-only and every path in validates, so there is no way to assemble
a configuration that parses but cannot run and have it survive as a
value. Two things are refused: a rate limit that would never admit a
call, because the decorators it configures reject a zero rate; and two
components in one lineup sharing a name, because a request selects
components by name, so the duplicate is unreachable and every
detection from either would carry the same attribution.

The one exception is deserializing through serde directly, which
constructs without checking — serde's error type cannot carry this
crate's error kinds, and validating there would report a semantically
impossible configuration as a syntax error. A host taking that path
calls `validate` itself.

## What's covered

Four component lineups, each selected per request by name or by tag:

- **NER** — BentoML-hosted models, and Pioneer's hosted GLiNER
  service.
- **LLM** — OpenAI, Anthropic, Gemini and Ollama, each attaching to
  the modalities its configuration names.
- **OCR** — BentoML, AWS Textract, and Google Cloud Document AI.
- **STT** — BentoML, and Deepgram's hosted API.

Beside the lineups: an availability restriction the operator sets and
a request cannot widen, a per-component rate and concurrency limit,
and the language candidates detection may return.

Every hosted third-party backend is behind its own off-by-default
feature, because each sends document content outside the deployment's
own infrastructure. A build that does not enable one cannot name it,
so deployment configuration alone cannot route documents to a service
the build was never meant to reach.

## Project

- **Changelog**: [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history
- **Contributing**: [CONTRIBUTING.md](../../CONTRIBUTING.md) for setup, local CI targets, and the pull-request process
- **License**: Apache 2.0, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
