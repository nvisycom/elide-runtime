# nvisy-cli

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

CLI entry point for the nvisy API server. Parses CLI flags +
`Nvisy.toml`, initialises tracing, and starts the HTTP server
provided by [`nvisy-server`](../nvisy-server).

## Feature Flags

Modality + provider features are forwarded to `nvisy-server` and
`nvisy-engine`. All bundled here are on by default; pare them down
with `--no-default-features` and an explicit `--features` list.

| Feature | Default | Description |
|---------|---------|-------------|
| `tabular` | yes | CSV + XLSX support |
| `image` | yes | PNG, JPEG, TIFF + OCR + VLM detection |
| `audio` | yes | WAV, MP3 + STT extraction |
| `rich` | yes | PDF + DOCX |
| `openai` | yes | OpenAI providers (GPT, Whisper STT) |
| `anthropic` | yes | Anthropic Claude completion provider |
| `google` | yes | Google Gemini completion provider |
| `bento` | yes | Externalised inference backends (NER + OCR) |

## Usage

```sh
# Run with the default feature set
cargo run -p nvisy-cli

# Run with only OpenAI and Anthropic, drop image/audio/rich
cargo run -p nvisy-cli --no-default-features \
    --features tabular,openai,anthropic

# Show all options
nvisy --help
```

## Configuration

CLI flags override the corresponding TOML fields; TOML overrides
built-in defaults. The TOML file path defaults to `./Nvisy.toml`
and can be overridden with `--config` or `NVISY_CONFIG`. See
[`Nvisy.example.toml`](../../Nvisy.example.toml) for the full
schema.

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
