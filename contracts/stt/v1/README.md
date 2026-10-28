# STT v1

Python's `elide_core.stt.v1` owns this contract. See the [generation workflow](../..).
`SttInput` is `{"request": {...}}`; `SttOutput` is the response object directly.

All wire timestamps and durations are integer milliseconds. The Rust adapter
converts segment and word timings to domain time spans, preserving text,
speaker labels, language, and available confidence. It rejects inverted timings
and values that cannot be converted into the domain's microseconds without
integer overflow. Zero-duration spans are valid, matching Python's validators.

Generated wire types retain model metadata, overall language, and duration even
where the Rust domain response has no corresponding field. Optional hints and
metadata accept omission or null. Fixtures cover speakers, word timings,
optional fields, zero-duration spans, and an empty transcript.
