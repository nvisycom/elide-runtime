# nvisy-cli

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

CLI entry point for the nvisy API server. Parses CLI flags +
`Nvisy.toml`, initialises tracing, and starts the HTTP server
provided by [`nvisy-server`](../nvisy-server).

## Overview

The binary is a thin wrapper: feature flags forward to `nvisy-server`
and on through to `nvisy-engine`, and runtime behaviour is driven
by `Nvisy.toml` (see [`Nvisy.example.toml`](../../Nvisy.example.toml)
for the full schema). CLI flags override the corresponding TOML
fields.

| Flag | Env | Default | Description |
|------|-----|---------|-------------|
| `--config` | `NVISY_CONFIG` | `Nvisy.toml` | TOML config file path |
| `--host` | `HOST` | `0.0.0.0` | Bind address |
| `--port` / `-p` | `PORT` | `8080` | Bind port |
| `--shutdown-timeout` | `SHUTDOWN_TIMEOUT` | `30` | Seconds to wait for graceful shutdown |
| `--data-dir` | `DATA_DIR` | `./data` | Data directory (content, contexts) |

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
