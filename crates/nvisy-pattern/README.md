# nvisy-pattern

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Built-in patterns, dictionaries, and validators for PII/PHI detection in the
Nvisy runtime.

## Overview

The crate ships a process-wide `PatternEngine` plus a `PatternEngineBuilder`
for custom subsets. Each scan runs four phases — regex pre-filtered by a
shared `RegexSet`, per-token glob matching via `globset`, dictionary lookup
via Aho-Corasick, and forced injection of `DenyList` values — into a single
deduplicated result.

Pattern definitions live as JSON under `assets/patterns/`, dictionaries
under `assets/dictionaries/`; both are embedded at compile time and
auto-discovered by `PatternRegistry` and `DictionaryRegistry`. Each JSON
pattern picks exactly one of `"regex" | "glob" | "dictionary"` as its
match source. For the JSON schema see the rustdoc on `JsonPattern`; for
per-field semantics see `RegexPattern`, `GlobPattern`, `DictionaryPattern`,
`ContextRule`, and `PatternMetadata`.

Per-scan filtering (allow-list suppression, deny-list forced detection,
context-aware confidence boosting) rides on `ScanContext`. The same context
also carries `extra_patterns: Vec<RuntimePattern>` for per-call ad-hoc
patterns (regex / glob / dictionary) — useful for tenant-supplied rules or
test injection without rebuilding the engine. Malformed extras surface
through `PatternEngine::try_scan_entities` as `ExtraPatternError`.

Post-match validation is keyed by name through `ValidatorResolver` and
referenced from regex patterns via `"validator": "<name>"`.

```rust,ignore
use nvisy_pattern::filter::{AllowList, DenyList, DenyRule, ScanContext};
use nvisy_pattern::{GlobPattern, MatchSource, PatternEngine, RuntimePattern};
use nvisy_ontology::entity::{EntityCategory, EntityKind, RecognitionMethod, ModelKind};

let mut deny = DenyList::new();
deny.insert("John Doe", DenyRule {
    category: EntityCategory::PersonalIdentity,
    entity_kind: EntityKind::PersonName,
    method: RecognitionMethod::ner("manual", ModelKind::SelfHosted),
});

let internal_invoice = RuntimePattern::new(
    "internal-invoice",
    EntityCategory::Financial,
    EntityKind::PaymentCard,
    MatchSource::Glob(GlobPattern {
        glob: "INV-*".into(),
        case_sensitive: true,
        confidence: 0.8,
    }),
);

let ctx = ScanContext {
    allow: ["123-45-6789", "000-00-0000"].into_iter().collect(),
    deny,
    extra_patterns: vec![internal_invoice],
    ..Default::default()
};

let (matches, errors) = PatternEngine::instance().try_scan_entities("...", &ctx);
```

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
