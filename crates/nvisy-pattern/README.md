# nvisy-pattern

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Built-in regex patterns and dictionaries for PII/PHI detection in the Nvisy runtime.

Patterns are JSON files under `assets/patterns/` that define a regex, an entity kind, a category, a confidence score, and an optional post-match validator. They are embedded at compile time and exposed through a lazy `PatternRegistry`.

Dictionaries are plain-text (`.txt`) or CSV (`.csv`) files under `assets/dictionaries/` containing matchable terms such as nationalities, religions, currencies, cryptocurrencies, and languages. They are similarly embedded and served via a `DictionaryRegistry`.

Both registries share the generic `Registry<V>` implementation for O(log n) lookup by name.
