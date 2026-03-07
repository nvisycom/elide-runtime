# nvisy-http

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Shared HTTP client with retry and tracing middleware for the Nvisy runtime.

Provides `HttpClient` and `HttpConfig` used by downstream crates
(`nvisy-ocr`, `nvisy-rig`, `nvisy-engine`) to share a single connection
pool with exponential-backoff retry and OpenTelemetry tracing
pre-installed.

## Usage

```rust
use nvisy_http::{HttpClient, HttpConfig};

// Default configuration (3 retries, 120s timeout, 10s connect, 90s idle)
let client = HttpClient::default();

// Custom configuration
let client = HttpClient::new(&HttpConfig {
    max_retries: 5,
    timeout_secs: 60,
    ..HttpConfig::default()
});
```

`HttpClient` implements `Deref<Target = ClientWithMiddleware>` so you
can call `.get()`, `.post()`, etc. directly on it.

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
