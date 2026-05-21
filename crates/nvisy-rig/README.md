# nvisy-rig

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

LLM agents over the [`rig`](https://github.com/0xPlaygrounds/rig)
framework for the Nvisy runtime. Hosts `BaseAgent` scaffolding plus
concrete agents for NER, computer vision, generation, OCR
verification, and speech (STT/TTS).

This crate replaces the LLM half of the former `nvisy-provider`. The
non-LLM OCR providers (AWS Textract, Google Vision, Azure DocAI,
Surya, PaddleX) live in `nvisy-ocr`.

## Modules

- **`agent::base`** — `BaseAgent`, `AgentConfig`, `AgentProvider`,
  context window, usage tracking.
- **`agent::generate`** — generic text generation.
- **`agent::ner`** — LLM-driven named-entity recognition. Consumed
  by `nvisy-detection::LlmRecognizer`.
- **`agent::cv`** — multimodal (image + text) LLM agents.
- **`agent::ocr`** — LLM-mediated entity verification for OCR
  output. Wraps an `nvisy_ocr::OcrEngine` for now; will be split
  in a follow-up so the verifier becomes a pure LLM agent and the
  OCR orchestration moves up to `nvisy-engine`.
- **`audio::stt`** / **`audio::tts`** — speech-to-text and
  text-to-speech via LLM providers (OpenAI Whisper, OpenAI TTS,
  etc.).

HTTP transport (`HttpClient`, `HttpConfig`, retry + tracing
middleware) lives in the shared `nvisy-http` crate; agents accept
clients built by callers.
