# nvisy-agent

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

LLM agents and orchestrating pipelines over the
[`rig`](https://github.com/0xPlaygrounds/rig) framework for the Nvisy
runtime. Provides typed agents for NER, CV classification, CV
verification, and synthetic-value generation, plus speech (STT/TTS)
services.

## Overview

Agents live under `agent::*`, grouped by modality. Each modality
bundles a detect-style and verify-style agent that share data shapes:

- `agent::ner` — `NerAgent` (detect candidates from text) +
  `NerVerifyAgent` (localize to byte offsets, optionally LLM-refine).
- `agent::cv` — `CvAgent` (classify pre-computed CV detections into
  entity categories) + `CvVerifyAgent` (validate upstream entity
  proposals against an image).
- `agent::generate` — `GenAgent` (synthetic replacement values for
  redaction).

Cross-cutting infrastructure (`AgentConfig`, `AgentProvider`,
`DetectionConfig`, `UsageStats`) is re-exported at `agent::*`.

Pipelines under `pipeline::*` compose agents into end-to-end flows
and own any cross-call state. `NerPipeline` chains
`NerAgent → NerVerifyAgent → coreference merge`; `CvPipeline` holds
the optional `CvAgent` and the always-present `CvVerifyAgent`,
exposing `classify` and `verify` as independent methods. Both
expose `reset()` (per-document state clear + cumulative usage zero)
and `usage()` (token totals since the last reset).

Audio services (`audio::stt::SttService`, `audio::tts::TtsService`)
wrap whisper / TTS providers behind their own `SttProvider` /
`TtsProvider` enums; same HTTP transport as the LLM agents.

HTTP transport (`HttpClient`, `HttpConfig`, retry + tracing
middleware) lives in the shared `nvisy-http` crate; rig agents
build their own clients internally from `AgentConfig::max_retries`.

```rust,ignore
use nvisy_agent::agent::{AgentConfig, AgentProvider, DetectionConfig};
use nvisy_agent::agent::ner::UnresolvedCandidatePolicy;
use nvisy_agent::pipeline::NerPipeline;

let provider = AgentProvider::openai("sk-...", "gpt-4o");
let pipeline = NerPipeline::new(
    &provider,
    AgentConfig::default(),
    None,                                    // no second-pass refiner
    UnresolvedCandidatePolicy::Drop,
)?;

let config = DetectionConfig::default();
let entities = pipeline.run("text to analyze", &config).await?;
let used = pipeline.usage();
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
