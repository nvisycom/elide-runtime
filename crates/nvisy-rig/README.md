# nvisy-rig

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

LLM/VLM-driven detection, redaction, and OCR backends for the Nvisy runtime.

Provides integration with large language models and vision-language models for entity detection, content redaction, and optical character recognition that supplements the rule-based detection layers.

## Providers

- **OpenAI**: GPT completion, Whisper speech-to-text, TTS
- **Anthropic**: Claude completion
- **Google**: Gemini completion

## Feature Flags

All provider features are disabled by default; enable only what you need.

| Feature | Default | Description |
|---------|---------|-------------|
| `openai-gpt` | no | Enable OpenAI GPT completion provider |
| `openai-whisper` | no | Enable OpenAI Whisper speech-to-text provider |
| `openai-tts` | no | Enable OpenAI text-to-speech provider |
| `anthropic-claude` | no | Enable Anthropic Claude completion provider |
| `google-gemini` | no | Enable Google Gemini completion provider |

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
