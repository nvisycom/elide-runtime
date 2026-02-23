# nvisy-pattern

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Built-in patterns, dictionaries, and validators for PII/PHI detection in the
Nvisy runtime.

## Architecture

Detection runs in three phases:

1. **Regex phase**: a `RegexSet` pre-filter identifies which compiled regexes
   may match, then each matching regex is run to extract offsets and values.
2. **Dictionary phase**: Aho-Corasick automata perform literal multi-pattern
   matching against known-value dictionaries.
3. **Deny-list phase**: known sensitive values not already matched by regex or
   dictionary are injected as synthetic matches with confidence `1.0`.

Allow-list filtering is applied inline during phases 1 and 2. All three phases
feed into a unified `Vec<PatternMatch>`.

### Pattern JSON schema

Patterns are JSON definition files embedded at compile time from
`assets/patterns/` and auto-discovered by `PatternRegistry`.

```json
{
  "name": "ssn",
  "category": "pii",
  "entity_type": "government_id",
  "pattern": {
    "regex": "\\b(\\d{3})-(\\d{2})-(\\d{4})\\b",
    "validator": "ssn"
  },
  "confidence": 0.9,
  "context": {
    "keywords": ["social security", "ssn", "tax id"],
    "window": 3,
    "boost": 0.1
  }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Unique pattern identifier |
| `category` | string | yes | Entity category (`pii`, `financial`, `credentials`) |
| `entity_type` | string | yes | Specific entity kind matching `EntityKind` |
| `pattern` | object | one of | Regex match source (mutually exclusive with `dictionary`) |
| `dictionary` | object | one of | Dictionary match source (mutually exclusive with `pattern`) |
| `confidence` | float | no | Base confidence score `[0.0, 1.0]`. Default: `1.0` |
| `context` | object | no | Co-occurrence context rule (see below) |

### `pattern` object (regex match source)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `regex` | string | — | Regular expression string |
| `validator` | string | — | Post-match validator name resolved via `ValidatorResolver` |
| `case_sensitive` | bool | `false` | Whether matching is case-sensitive |

### `dictionary` object (dictionary match source)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | — | Named dictionary from `DictionaryRegistry` |
| `case_sensitive` | bool | `false` | Whether matching is case-sensitive |

### Context rule (co-occurrence scoring)

The optional `context` block enables span-level confidence boosting. When a
match is found, nearby spans (controlled by `window`) are searched for any of
the `keywords`. If at least one keyword is present, the match confidence is
increased by `boost`, clamped to `[0.0, 1.0]`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `keywords` | string[] | — | Strings to search for in nearby spans |
| `window` | int | `3` | Number of spans before/after the match to examine |
| `boost` | float | `0.1` | Confidence increase when a keyword is found |
| `case_sensitive` | bool | `false` | Whether keyword matching is case-sensitive |

Co-occurrence scoring is applied at the detection layer level (in
`nvisy-detection`), not inside `PatternEngine::scan_text`, because the engine
operates on one span at a time while co-occurrence needs visibility across
adjacent spans.

## Allow/deny lists

The `PatternEngineBuilder` supports exact-match allow and deny lists via the
[`AllowList`] and [`DenyList`] types:

```rust,ignore
let allow = AllowList::new()
    .with("123-45-6789")             // suppress known test SSN
    .with("000-00-0000");

let deny = DenyList::new()
    .with("John Doe", EntityCategory::Pii, EntityKind::PersonName);

let engine = PatternEngine::builder()
    .with_allow(allow)
    .with_deny(deny)
    .build()?;
```

- **Allow list** (`AllowList`): matched values that appear in the allow list
  are silently dropped during `scan_text`.
- **Deny list** (`DenyList`): if a deny-list value is found in the text but
  was not matched by any regex or dictionary pattern, it is injected as a
  synthetic `PatternMatch` with confidence `1.0` and source
  `DetectionSource::DenyList`.

Both types implement `FromIterator` for easy construction from iterators.

## Validators

Validators are post-match checks resolved by name through `ValidatorResolver`.
Regex patterns reference a validator by name in their `pattern.validator` field;
the engine runs the validator on each raw match and drops values that fail.

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
