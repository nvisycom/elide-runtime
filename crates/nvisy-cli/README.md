# nvisy-cli

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

CLI entry point for the nvisy API server. Parses command-line arguments, initialises tracing, and starts the HTTP server provided by [`nvisy-server`](../nvisy-server).

## Feature Flags

Provider features are forwarded from [`nvisy-server`](../nvisy-server). All are enabled by default.

| Feature | Default | Description |
|---------|---------|-------------|
| `openai` | yes | Enable all OpenAI providers (GPT, Whisper STT, TTS) |
| `anthropic` | yes | Enable Anthropic Claude completion provider |
| `google` | yes | Enable Google Gemini + Google Cloud Vision OCR |
| `microsoft` | yes | Enable Azure Document Intelligence OCR |
| `amazon` | yes | Enable AWS Textract OCR |

## Usage

```sh
# Run with all providers (default)
cargo run -p nvisy-cli

# Run with only OpenAI and Anthropic
cargo run -p nvisy-cli --no-default-features --features openai,anthropic

# Show all options
nvisy-server --help
```

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
