# Bringing your own inference

The shipped services in [`packages/`](../packages) are one implementation of a
wire contract, not the contract itself. Any HTTP service reproducing it is a
drop-in replacement — self-hosted weights, a different model family, Triton
behind an adapter, or something hand-rolled.

A replacement needs no BentoML and no Python. Only the wire shape matters.

## The contract

| Replace | Endpoint | Request body | Reference models |
| :-- | :-- | :-- | :-- |
| [NER](../packages/elide-ner) | `POST /recognize` | `{"requests": [...]}` | [`elide_core.ner.v1`](../packages/elide-core/src/elide_core/ner/v1) |
| [OCR](../packages/elide-ocr) | `POST /recognize` | `{"requests": [...]}` | [`elide_core.ocr.v1`](../packages/elide-core/src/elide_core/ocr/v1.py) |
| [STT](../packages/elide-stt) | `POST /transcribe` | `{"request": {...}}` | [`elide_core.stt.v1`](../packages/elide-core/src/elide_core/stt/v1.py) |

Each service's README documents its endpoint with a worked request and
response.

## Three things that are easy to get wrong

**The request envelopes differ.** NER and OCR batch concurrent calls, so their
bodies wrap a **list** under `requests`. STT does not — a transcript can take
minutes, and batching would let one long clip hold short ones behind it — so it
takes a single object under `request`. Sending the wrong envelope returns a
`400` naming the missing field.

**Field names are camelCase on the wire**, not the `snake_case` the Python
models use internally. Pydantic's `alias_generator=to_camel` and serde's
`rename_all = "camelCase"` agree on this; a hand-rolled server must match it.

**The Rust adapters are the source of truth, not the Python models.**
[`elide-bentoml`](../crates/adapter/elide-bentoml)'s serde definitions decide
what goes over the wire; the pydantic models in
[`elide-core`](../packages/elide-core) mirror them. A change made only on the
Python side produces a contract nothing speaks.

## Health

Every service serves two endpoints beyond its own:

- `GET /healthz` — liveness. The process is up.
- `GET /readyz` — readiness. The **model is loaded**, which can lag liveness by
  minutes on a cold start.

Gate traffic on `/readyz`. The compose setup in [`docker/`](../docker/README.md)
does, which is why a service there is `starting` rather than `healthy` while it
loads.

## Pointing a deployment at a replacement

By URL, with no code change — the backend is configuration:

```toml
[[recognizers.ner]]
name = "my-own-ner"
kind = "bento"
base_url = "http://my-own-ner:8080"
model = "whatever-you-run"
```

`kind = "bento"` selects the client that speaks this contract; it says nothing
about what runs behind the URL. The same applies to `[[enrichers.ocr]]` and
`[[enrichers.stt]]`.
