# nvisy-http

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Shared HTTP client for the Nvisy runtime — a thin newtype over
`reqwest_middleware::ClientWithMiddleware` with exponential-backoff
retry and OpenTelemetry tracing layers pre-installed.

## Overview

`HttpClient` wraps a middleware-configured `reqwest` client; consumers
build their own from `HttpConfig` at agent/backend construction time
(no global state). `nvisy-agent` (LLM agents, STT/TTS) and `nvisy-ocr`
(OCR providers) both depend on this crate so the retry policy and
tracing layer are identical across every outbound HTTP call.

`HttpConfig` carries durations + retry count and deserializes via
`humantime_serde` so config files accept `"120s"`, `"2min"`, etc.

`RequestBuilderExt` adds `.send_and_check("provider")` and
`.send_and_parse::<T>("provider")` helpers that map transport and
status errors to `nvisy_core::Error` with consistent retryability
classification (5xx + network errors are retryable; 4xx are not).

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
