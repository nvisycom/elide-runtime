# nvisy-agent

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

LLM agents and orchestrating pipelines over the
[`rig`](https://github.com/0xPlaygrounds/rig) framework for the Nvisy
runtime. Provides typed agents for text-side NER and image-side VLM
detection + verification, plus a speech-to-text service.

## Overview

`agent::*` groups concrete agents by modality. Each modality bundles
a detect-style and verify-style agent that share data shapes:

- `agent::ner` — `NerAgent` (LLM detector that produces typed
  candidates from text) + `NerVerifyAgent` (whole-audit
  confirm/correct/reject pass over a merged entity set) +
  `UnresolvedCandidatePolicy` for handling candidates that can't be
  uniquely localized to source offsets.
- `agent::vlm` — `VlmAgent` (vision-language model detector that
  emits image-coordinate entities with bounding boxes) +
  `VlmVerifyAgent` (per-entity confirm/correct/reject against the
  source image) + `VerificationCandidate` (entity + resolved value)
  for verifier input.

Cross-cutting infrastructure (`AgentConfig`, `AgentProvider`,
`LlmNerContext`, `VlmDetectContext`, `NerHint`, `UsageStats`) is
re-exported at `agent::*`.

`pipeline::*` composes agents into per-modality pipelines that own
the cross-call state (token usage tracking, optional verify-pass
chaining):

- `LlmNerPipeline::new(provider, detect_cfg, verify_cfg,
  unresolved_policy)` — both detect and verify configs are
  independently optional; at least one must be `Some`.
- `VlmPipeline::new(provider, detect_cfg, verify_cfg)` — same
  presence-and-flag pattern. Exposes `detect` and `verify` as
  independent async methods.

Both pipelines expose `reset()` (zero the cumulative usage tracker
between documents).

`audio::stt::SttService` wraps Whisper-family STT providers behind
its own `SttProvider` enum; shares the rig HTTP transport with the
LLM agents.

## Feature Flags

Provider features are independently selectable. None are on by
default — the engine/server entry points opt in.

| Feature | Default | Description |
|---------|---------|-------------|
| `openai-gpt` | no | OpenAI completion provider (used by NER + VLM agents) |
| `openai-whisper` | no | OpenAI Whisper STT provider |
| `anthropic-claude` | no | Anthropic Claude completion provider |
| `google-gemini` | no | Google Gemini completion provider |

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
