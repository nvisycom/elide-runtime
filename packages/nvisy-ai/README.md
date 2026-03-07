# nvisy-hf

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Hugging Face model runner for the Nvisy runtime. Called from Rust via the [`nvisy-python`](../../crates/nvisy-python) PyO3 bridge.

Runs local transformer models for NER, text classification, and token classification tasks that supplement the API-based providers in [`nvisy-rig`](../../crates/nvisy-rig).

## Requirements

- Python >= 3.11
- `transformers`
- `torch`

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
