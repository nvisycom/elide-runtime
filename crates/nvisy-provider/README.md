# nvisy-provider

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

External service provider integrations for the Nvisy runtime. Combines HTTP client infrastructure, OCR backends, LLM agents, and speech services into a single crate.

- **HTTP client**: shared `HttpClient` with retry (exponential backoff) and tracing middleware
- **OCR**: type-erased `OcrEngine` with pluggable backends (AWS Textract, Google Vision, Azure Document Intelligence, Surya, PaddlePaddle)
- **LLM agents**: NER, computer vision, OCR verification, and context generation agents backed by configurable LLM providers (OpenAI, Anthropic, Google Gemini)
- **Speech**: speech-to-text and text-to-speech service traits with provider implementations (OpenAI Whisper, OpenAI TTS)

## Feature flags

| Flag | Description |
|------|-------------|
| `aws-textract` | Enable AWS Textract OCR provider |
| `google-vision` | Enable Google Cloud Vision OCR provider |
| `azure-docai` | Enable Azure Document Intelligence OCR provider |
| `openai-gpt` | Enable OpenAI GPT completion provider |
| `openai-whisper` | Enable OpenAI Whisper STT provider |
| `openai-tts` | Enable OpenAI TTS provider |
| `anthropic-claude` | Enable Anthropic Claude completion provider |
| `google-gemini` | Enable Google Gemini completion provider |

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
