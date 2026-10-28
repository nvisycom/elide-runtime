# NER v1 contract

The Pydantic models in `packages/elide-core/src/elide_core/ner/v1/` own the
HTTP contract. `contract.py` adds the BentoML request object and response array.

Regenerate from the repository root:

```sh
uv run python scripts/gen_contracts.py
uv run python scripts/gen_contracts.py --check
```

This exports request/response JSON Schema (2020-12), shared fixtures, and the
Rust wire types in `crates/adapter/elide-bentoml/src/ner/wire.rs`. Commit all
outputs together. CI checks regeneration and runs both languages' contract tests.
Rust types are generated using pinned Typify; see the [shared workflow](../..).
Field constraints supported by the generated types are enforced there; cross-field
validators and domain conversions remain in the handwritten adapter.

Offsets are Unicode code-point indices into the original text, inclusive start
and exclusive end. No Unicode normalization is performed. The Rust adapter
converts these into UTF-8 byte ranges, checking bounds and matched text. Fixtures
cover ASCII, accented text, supplementary-plane characters, CJK, and combining
characters. Cross-field validation and model-specific input budgets require
behavioral tests; JSON Schema and generated structs alone cannot enforce them.

The Rust adapter requires explicit labels. Missing labels fail locally because
this service has no implicit fixed label set. An explicitly empty label list
returns an empty result without HTTP; it is never sent as an invalid schema.
Metadata stays in the wire response even where the Rust domain response has no
corresponding field. Optional fields may be omitted or null. Additive optional
fields can remain v1; breaking wire changes require a new contract version.
