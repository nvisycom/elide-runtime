# elide-provider

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Deployment-configured recognition and redaction providers for the
Elide Runtime.

## Overview

A deployment decides some things once: which NER model, which LLM
provider, and which OCR and STT engines. ProviderConfig is that set as
one serializable value, and building it yields a Provider.

A Provider then builds an Orchestrator per request, because an
orchestrator carries request data the policies in force, the caller's
scope and key, a correlation id that no deployment-wide value could
hold. Config is parsed once; an orchestrator is constructed per
request.

This crate depends only on Elide and the governance vocabulary, never
on the pipeline: it is the half that turns configuration into Elide
runtime values, and knows nothing about documents, audits, or reviewer
decisions. A host reads a file, an environment variable, or an
encrypted row in its own database, fills in a ProviderConfig, and
builds. Neither crate learns which.

Backend credentials travel inside the backend configs, because that is
where Elide's own provider types keep them: an LLM recognizer names its
model and its API key together. A serialized config therefore contains
credentials and belongs wherever the deployment already keeps secrets.
Cryptographic keys are not in the provider config at all. A key
belongs to the caller asking for redaction, not to the process serving
them, so it travels on a RequestContext with the request that needs it.
One provider then serves many callers, each with its own key.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
