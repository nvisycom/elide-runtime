# nvisy-identify

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Detection orchestration, entity ontology, and policy evaluation for the Nvisy runtime.

Organized by **detection method** rather than content modality:

- **pattern/** — Deterministic regex and dictionary matching via `PatternEngine`
- **ner/** — Statistical NLP named-entity recognition (text and image)
- **llm/** — LLM-based contextual entity detection via `LlmBackend`
- **vision/** — Computer vision layers (face, object, OCR detection)
- **audio/** — Audio detection via transcription + NER pipeline
- **fusion/** — Post-detection entity merging, deduplication, and ensemble scoring
- **policy/** — Policy evaluation, governance rules, and audit trails

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
