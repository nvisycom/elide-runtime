# @nvisy/plugin-core

Core plugin for the Nvisy runtime with built-in chunking, partitioning, loading, and text splitting.

## Install

```bash
npm install @nvisy/plugin-core
```

## Plugin Registration

```ts
import { corePlugin } from "@nvisy/plugin-core";

// Register with the engine
registry.load(corePlugin);
```

The `corePlugin` registers:

- **Datatype**: `Document`, `Blob`, `Chunk`, `Embedding`
- **Actions**: `chunkSimple`, `partition`
- **Loaders**: `plaintextLoader`, `csvLoader`, `jsonLoader`

## Actions

### `chunkSimple`

Splits documents into smaller chunks. Accepts a `strategy` discriminator to select the splitting method.

**Character strategy** — fixed-size windows with optional overlap:

```ts
{ strategy: "character", maxCharacters: 500, overlap: 50 }
```

**Section strategy** — split on markdown headings:

```ts
{ strategy: "section", level: 2, maxCharacters: 1000, combineUnder: 200 }
```

**Page strategy** — split on page boundaries (`\f`, `---`, `***`) or structured page elements:

```ts
{ strategy: "page", maxCharacters: 2000 }
```

### `partition`

Partitions documents into multiple documents with metadata tracking.

**Auto strategy** — pass-through, preserves content as-is:

```ts
{ strategy: "auto" }
```

**Rule strategy** — split on a regex pattern:

```ts
{ strategy: "rule", pattern: "\\n{2,}", includeDelimiter: false, inferTableStructure: false }
```

## Loaders

### `plaintextLoader`

Converts `.txt` blobs into documents.

| Parameter | Type | Default |
|-----------|------|---------|
| `encoding` | `"utf-8" \| "ascii" \| "latin1" \| "utf16le"` | `"utf-8"` |

### `csvLoader`

Converts `.csv` / `.tsv` blobs into documents. Rows are formatted as `column: value` when headers are present.

| Parameter | Type | Default |
|-----------|------|---------|
| `delimiter` | `string` | `","` |
| `hasHeader` | `boolean` | `true` |
| `encoding` | `"utf-8" \| "ascii" \| "latin1" \| "utf16le"` | `"utf-8"` |

### `jsonLoader`

Converts `.json` / `.jsonl` / `.ndjson` blobs into documents. Scalar object fields are extracted as document metadata.

| Parameter | Type | Default |
|-----------|------|---------|
| `encoding` | `"utf-8" \| "ascii" \| "latin1" \| "utf16le"` | `"utf-8"` |

## Splitters

Reusable `string → string[]` splitting utilities, usable independently of the action system.

### `splitByDelimiter`

Split text on a literal string delimiter.

```ts
import { splitByDelimiter } from "@nvisy/plugin-core";

splitByDelimiter("a---b---c", { delimiter: "---" });
// → ["a", "b", "c"]

splitByDelimiter("a---b---c", { delimiter: "---", keepDelimiter: true });
// → ["a", "---b", "---c"]
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `delimiter` | `string` | — | String to split on |
| `keepDelimiter` | `boolean` | `false` | Prepend delimiter to subsequent segments |
| `trimEmpty` | `boolean` | `true` | Discard empty/whitespace-only segments |

### `splitByRegex`

Split text on a regex pattern (compiled with `gm` flags).

```ts
import { splitByRegex } from "@nvisy/plugin-core";

splitByRegex("intro\n## A\ncontent A\n## B\ncontent B", { pattern: "^## .+$", keepSeparator: true });
// → ["intro\n", "## A\ncontent A\n", "## B\ncontent B"]
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `pattern` | `string` | — | Regex pattern to split on |
| `keepSeparator` | `boolean` | `false` | Keep matched separator at start of segments |
| `trimEmpty` | `boolean` | `true` | Discard empty/whitespace-only segments |

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License - see [LICENSE.txt](../../LICENSE.txt)
