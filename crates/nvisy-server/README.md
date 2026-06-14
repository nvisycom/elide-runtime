# nvisy-server

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

HTTP front-end for the Nvisy runtime: thin REST surface over
`nvisy-engine`'s two-phase detect/redact pipeline with an
auto-generated OpenAPI spec and Scalar reference UI.

## Overview

Built on Axum with Aide for OpenAPI generation and Scalar for the
docs UI. Handlers compose into versioned route trees under
`/api/v1` and translate request payloads into the typed
`nvisy-engine` inputs — `NewDetection` carries inline `Policy`
bodies straight into `DetectionInput`, `NewRedaction` references a
prior detection by id, and file routes stream raw bytes through
the registry's content store. Middleware layers — request id
propagation, structured tracing, sensitive-header redaction, CORS,
compression, body-size limits, and the OpenAPI finaliser — are
applied outside-in via extension traits on `ApiRouter` / `Router`.
Bind address, TLS, and feature toggles live on `nvisy-cli`; the
server crate is purely the request/response surface.

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
