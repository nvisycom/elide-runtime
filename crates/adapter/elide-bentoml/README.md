# elide-bentoml

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Shared BentoML HTTP client wrapper for elide backends.

## Overview

Per-modality backends (NER, OCR, STT) implement the traits elide
defines for each — `elide-ner`, `elide-image::ocr`, `elide-audio::stt` —
behind a feature apiece, over a common HTTP client, params validation
and error translation. The Python services
this client talks to live under [`packages/`](../../../packages) in the
same repository.

## End-to-end tests

The Rust adapters can be tested over real HTTP against the production Python
BentoML applications. Only the model libraries are replaced by deterministic
fakes; request validation, adaptive dispatch, image/audio decoding, backend
projection, and Rust domain conversion all run normally. No model weights or
GPU are needed.

```sh
cargo test --locked -p elide-bentoml --test ner --test ocr --test stt -- --ignored
```

A running Docker daemon is required. Testcontainers builds a shared test image
from the `e2e` dependency group of the root tooling project's lock, then starts one isolated container
per service. It manages ports, readiness, logs, and cleanup. A local Python
environment is not needed. Docker caches dependency layers across runs; the
image is rebuilt from the current checkout for each test run.

These tests are explicitly enabled because ordinary Rust tests do not require
Docker. The wire-contract workflow runs them in CI.

Coverage includes canonical/localized NER labels and Unicode offsets, batch
ordering, OCR confidence filtering and pixel geometry, STT millisecond timings
and word data, and service validation errors. Existing real-model Python tests
remain separate; these E2E tests verify integration rather than model accuracy.

## Project

- **Changelog**: [CHANGELOG.md](../../../CHANGELOG.md) for release notes and version history
- **Contributing**: [CONTRIBUTING.md](../../../CONTRIBUTING.md) for setup, local CI targets, and the pull-request process
- **License**: Apache 2.0, see [LICENSE](../../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
