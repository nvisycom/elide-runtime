# elide-pioneer

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-provider/rust-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-provider/actions/workflows/rust-build.yml)

Pioneer (Fastino) GLiNER2-backed NER backend for elide.

## Overview

A `NerBackend` over [Pioneer](https://pioneer.ai)'s hosted GLiNER2 API —
the same model family the self-hosted
[`bento-gliner2`](../../packages/bento-gliner2) service runs locally, so
the two are directly comparable. A sibling to
[`elide-bentoml`](../elide-bentoml), not a part of it: both implement the
same trait, so a deployment picks one without either knowing about the
other.

Pioneer publishes no Rust SDK, so this speaks to `POST /gliner-2`
directly over `reqwest`: one request, `X-API-Key` for auth, and a JSON
body carrying the text, the labels to extract, and a confidence
threshold. `include_confidence` and `include_spans` are always set —
without them the response carries neither a score nor an offset, which
are exactly what a `NerSpan` is made of.

Labels arrive per call, so extraction is zero-shot: any label the caller
names is extracted, not a fixed taxonomy. elide's `Label` can carry a
natural-language description to steer a zero-shot model, but Pioneer's
wire schema is a list of bare names with no slot for one, so descriptions
are dropped.

**Offsets are converted, not copied.** Pioneer reports *character*
positions; `NerSpan::offset` is a byte range into the source string. The
two coincide only for ASCII, so a document containing an accented name
would otherwise have every following span silently shifted — in a
redaction pipeline, redacting the wrong bytes. A span that does not
resolve against the submitted text, or whose length disagrees with the
matched text Pioneer echoes back, is dropped rather than repaired.

## Before pointing this at production text

**This sends the text to be scanned to a third party** — for a redaction
pipeline, the un-redacted original. The self-hosted `bento-gliner2`
service runs the same open-weight model
(`fastino/gliner2-privacy-filter-PII-multi`, Apache-2.0) with no egress,
so this backend buys convenience rather than capability.

Pioneer's own published terms make that a live concern. As of September
2026, from their Trust & Safety documentation:

| | Their wording |
|---|---|
| Retention | "API inputs and outputs are retained indefinitely" by default, tiering "to colder storage over time but ... not automatically deleted". Per-request `store: false` gives "full zero-retention handling" — but only "for eligible use cases", which are not enumerated. |
| Platform training | "Sometimes, by default. Unless you opt out, we may use inputs and outputs you send through the API to improve and train our models." The opt-out is an Enterprise setting. |
| Task-model training | Separate, and not covered by that opt-out: inference data "is used for continuous adaptation and remediation training of your own task models ... regardless of the platform-training opt-out". |
| DPA | "At this time, we do not offer a Data Processing Addendum (DPA)." |
| SOC 2 / ISO 27001 | Both in progress; a report is expected around November 2026. |
| HIPAA | No BAA documented at any tier. |
| Sub-processors | 15, all US-based, list last updated July 2026. It includes OpenAI and Anthropic for AI/ML services; whether GLiNER2 calls specifically stay off that path is not documented. |

Two controls do exist, so this is a configuration decision rather than a
flat no: `store: false` per request, if your use case is eligible, and
the Enterprise platform-training opt-out. Neither reaches the task-model
training, which their own page says continues regardless — so on the
published terms there is no documented way to stop submitted text from
training anything at all.

`with_zero_retention()` sends `store: false` on every request. It is off
by default, so a deployment gets Pioneer's own behaviour unless it opts
in, and it is a request rather than a guarantee: eligibility is theirs to
decide, and it does not reach the task-model training above.

For a de-identification product that is a decision to take deliberately.
Confirm the terms directly rather than from this table: it was written
against their Trust & Safety page in September 2026 and will age.

An enterprise VPC deployment is mentioned in their materials but is
sales-gated and undocumented. Since the model weights are already
Apache-2.0 on Hugging Face, that is the variant worth asking about: it
would make this backend a genuine alternative rather than a trade.

## Two known API quirks

The `gliner2` Python client still hardcodes `https://api.fastino.ai`,
whose TLS certificate expired in October 2025. The live endpoint is
`https://api.pioneer.ai`, which is what this crate defaults to.

Pioneer's docs show GLiNER2 called through an OpenAI-compatible
`/v1/chat/completions` route with `model: "fastino/gliner2-large-v1"`,
but that model does not appear in a live `GET /v1/models` listing. This
crate uses the dedicated `/gliner-2` route, which is what their own
Python client calls and what their OpenAPI spec types.

## License

Apache 2.0 License, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-provider/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
