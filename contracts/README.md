# Python-owned wire contracts

The versioned Pydantic models in `packages/elide-core/src/elide_core/` define the
NER, OCR, and STT HTTP contracts. Each service exposes a consistently named
`NerInput/NerOutput`, `OcrInput/OcrOutput`, or `SttInput/SttOutput` envelope.
All JSON field names use snake_case, matching Python and Rust.
These names do not add JSON wrappers: NER/OCR outputs are arrays; STT output is
one object. Payload models retain names such as `NerRequest` and `SttResponse`.

Rust code generation uses [Typify](https://github.com/oxidecomputer/typify).
Install the pinned generator and repository formatter once:

```sh
cargo install cargo-typify --version 0.5.0 --locked
rustup toolchain install nightly --component rustfmt
```

Regenerate all schemas, shared fixtures, and Rust wire types from the repo root:

```sh
uv run python scripts/gen_contracts.py
uv run python scripts/gen_contracts.py --check
```

Keep this directory and generated `wire.rs` files tracked. They are reviewable
contract snapshots, not disposable build output. CI fails if regeneration
changes them and runs Python and Rust tests against the same fixtures. Ordinary
Rust builds use the committed wire types and do not require Python.

Python exports canonical JSON Schema 2020-12. A small compatibility conversion
renames definition and tuple keywords for Typify's Draft 7 reader; the tracked
schemas remain unchanged. Typify owns Rust type generation, including string
enums, nonempty strings, positive integers, and fixed-size geometry tuples.
Handwritten adapters own domain conversion and cross-field checks that schemas
cannot capture, such as Unicode offsets and timing order. Breaking wire changes
require a new contract version.

- [NER v1](ner/v1): Unicode character indices converted to UTF-8 byte ranges.
- [OCR v1](ocr/v1): pixel geometry, polygons, and hierarchy flattened to regions.
- [STT v1](stt/v1): integer millisecond timings, optional speakers and words.
