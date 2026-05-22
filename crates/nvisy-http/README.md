# nvisy-http

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Shared HTTP client for the Nvisy runtime. A thin newtype over
`reqwest_middleware::ClientWithMiddleware` with exponential-backoff
retry and OpenTelemetry tracing layers pre-installed.

Used by `nvisy-rig` (LLM agents, STT/TTS) and `nvisy-ocr` (OCR
providers). Both crates construct their own clients from
[`HttpConfig`] at agent/backend build time — no global state.

## Public surface

- [`HttpClient`] — newtype wrapping a configured middleware client.
- [`HttpConfig`] — durations + retry count, serde with
  `humantime_serde` so config files accept `"120s"`, `"2min"`, etc.
- [`RequestBuilderExt`] — `.send_and_check("provider")` and
  `.send_and_parse::<T>("provider")` helpers that map transport and
  status errors to `nvisy_core::Error` with consistent retryability
  classification.
