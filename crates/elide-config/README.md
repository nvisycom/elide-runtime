# elide-config

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Deployment configuration for the Elide Runtime engine: backends,
credentials, and key providers.

## Overview

An engine is assembled once at startup from things a deployment owns:
which NER model, which LLM provider, which OCR and STT engines, and the
cryptographic key provider the HmacHash and Encrypt operators resolve
through. EngineConfig is that set as one serializable value, and
building it yields a running engine.

This lives apart from elide-pipeline so the engine crate holds the
running engine and nothing about where its configuration came from. A
host reads a file, an environment variable, or an encrypted row in its
own database, fills in an EngineConfig, and builds. The pipeline never
learns which.

Backend credentials travel inside the backend configs, because that is
where Elide's own provider types keep them: an LLM recognizer names its
model and its API key together. A serialized config therefore contains
credentials and belongs wherever the deployment already keeps secrets.
Key material is the exception and is deliberately not a field: the
config names the shape of provider to build, and the bytes are passed
separately, so they have no path into a serialized config at all.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
