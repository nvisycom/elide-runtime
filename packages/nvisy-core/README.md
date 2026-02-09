# @nvisy/core

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Core primitives and abstractions for the Nvisy runtime platform.

## Features

- **Data types**: `Document`, `Chunk`, `Embedding`, and `Blob` for pipeline data, with lineage tracking via `Data` base class
- **Document model**: structured `Element` hierarchy with typed subclasses (`ImageElement`, `TableElement`, `FormElement`, `EmailElement`, `CompositeElement`) and provenance metadata
- **Plugin system**: bundle providers, streams, actions, loaders, and custom datatypes under a namespace
- **Provider abstraction**: connection lifecycle management with credential validation
- **Stream contracts**: resumable sources and sinks for external systems
- **Action contracts**: stream transforms with optional client dependencies
- **Loader contracts**: `Blob` to `Document` transforms with file extension and MIME type matching
- **Error taxonomy**: `RuntimeError`, `ValidationError`, `ConnectionError`, `CancellationError`, `TimeoutError`

## Overview

This package defines the foundational abstractions that all Nvisy plugins implement:

- **Data types** (`Data`, `Document`, `Chunk`, `Embedding`, `Blob`): immutable data containers that flow through pipelines. All extend `Data`, which provides `id`, `parentId`, `metadata`, and lineage methods (`deriveFrom`, `withParent`).
- **Elements** (`Element`, `ImageElement`, `TableElement`, etc.): structured content within documents, with typed subclasses for images, tables, forms, and emails. Includes provenance metadata, coordinate systems, and an element type ontology.
- **Plugins** (`Plugin.define`): namespace for grouping providers, streams, actions, loaders, and custom datatypes.
- **Providers** (`Provider.withAuthentication`, `Provider.withoutAuthentication`): external client lifecycle management.
- **Streams** (`Stream.createSource`, `Stream.createTarget`): data I/O layer for reading from and writing to external systems.
- **Actions** (`Action.withClient`, `Action.withoutClient`): stream transforms that process data between sources and targets.
- **Loaders** (`Loader.define`): specialized transforms that convert `Blob` objects into `Document` instances, matched by file extension and MIME type.

## Usage

### Defining a Provider

```ts
import { Provider } from "@nvisy/core";
import { z } from "zod";

const credentialSchema = z.object({
  apiKey: z.string(),
  endpoint: z.string().url(),
});

const myProvider = Provider.withAuthentication("my-provider", {
  credentials: credentialSchema,
  connect: async (creds) => {
    const client = await createClient(creds);
    return {
      client,
      disconnect: () => client.close(),
    };
  },
});
```

### Defining a Stream Source

```ts
import { Stream, Document } from "@nvisy/core";
import { z } from "zod";

const contextSchema = z.object({ cursor: z.string().optional() });
const sourceParamSchema = z.object({ limit: z.number() });

const mySource = Stream.createSource("my-source", MyClient, {
  type: Document, context: contextSchema, params: sourceParamSchema,
  reader: async function* (client, ctx, params) {
    for await (const item of client.list({ cursor: ctx.cursor, limit: params.limit })) {
      yield { data: new Document(item.text), context: { cursor: item.id } };
    }
  },
});
```

### Defining a Stream Target

```ts
import { Stream, Embedding } from "@nvisy/core";
import { z } from "zod";

const targetParamSchema = z.object({ collection: z.string() });

const myTarget = Stream.createTarget("my-target", MyClient, {
  type: Embedding, params: targetParamSchema,
  writer: (client, params) => async (item) => {
    await client.insert(params.collection, item);
  },
});
```

### Defining an Action

```ts
import { Action, Document, Chunk } from "@nvisy/core";
import { z } from "zod";

const chunkerParamSchema = z.object({ maxLength: z.number() });

const myChunker = Action.withoutClient("my-chunker", {
  types: [Document, Chunk],
  params: chunkerParamSchema,
  transform: async function* (stream, params) {
    for await (const doc of stream) {
      for (let i = 0; i < doc.content.length; i += params.maxLength) {
        yield new Chunk(doc.content.slice(i, i + params.maxLength)).deriveFrom(doc);
      }
    }
  },
});
```

### Defining a Loader

```ts
import { Loader, Document } from "@nvisy/core";
import { z } from "zod";

const loaderParamSchema = z.object({
  encoding: z.enum(["utf-8", "ascii"]).default("utf-8"),
});

const myLoader = Loader.define("markdown", {
  extensions: [".md", ".markdown"],
  contentTypes: ["text/markdown"],
  params: loaderParamSchema,
  load: async function* (blob, params) {
    const text = blob.data.toString(params.encoding);
    yield new Document(text).deriveFrom(blob);
  },
});
```

### Defining a Datatype

Custom data types extend the `Data` base class and are registered with `Datatype.define`. All `Data` subclasses get a unique `id`, optional `metadata`, and lineage tracking via `deriveFrom` / `withParent`.

```ts
import { Data, Datatype } from "@nvisy/core";

class Audio extends Data {
  readonly #duration: number;
  readonly #sampleRate: number;

  constructor(duration: number, sampleRate: number) {
    super();
    this.#duration = duration;
    this.#sampleRate = sampleRate;
  }

  get duration(): number {
    return this.#duration;
  }

  get sampleRate(): number {
    return this.#sampleRate;
  }
}

const audioDatatype = Datatype.define("audio", Audio);
```

### Bundling into a Plugin

```ts
import { Plugin, Datatype, Document, Chunk } from "@nvisy/core";

const myPlugin = Plugin.define("my-plugin")
  .withDatatypes(audioDatatype)
  .withProviders(myProvider)
  .withStreams(mySource, myTarget)
  .withActions(myChunker)
  .withLoaders(myLoader);
```

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License - see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
