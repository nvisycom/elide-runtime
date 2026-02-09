import { z } from "zod";
import type { JsonValue } from "../src/datatypes/data.js";
import { Data } from "../src/datatypes/data.js";
import { Provider } from "../src/provider.js";
import type { Resumable } from "../src/stream.js";
import { Stream } from "../src/stream.js";

/** Minimal row-like data type for testing. */
export class TestRow extends Data {
	readonly #columns: Readonly<Record<string, JsonValue>>;

	constructor(columns: Record<string, JsonValue>) {
		super();
		this.#columns = columns;
	}

	get columns(): Readonly<Record<string, JsonValue>> {
		return this.#columns;
	}

	get(column: string): JsonValue | undefined {
		return this.#columns[column];
	}
}

export const Credentials = z.object({
	host: z.string(),
	port: z.number(),
});
export type Credentials = z.infer<typeof Credentials>;

export const Params = z.object({
	table: z.string(),
});
export type Params = z.infer<typeof Params>;

export const Cursor = z.object({
	offset: z.number(),
});
export type Cursor = z.infer<typeof Cursor>;

export class ExampleClient {
	readonly rows: ReadonlyArray<Record<string, JsonValue>> = [
		{ id: "1", name: "Alice" },
		{ id: "2", name: "Bob" },
		{ id: "3", name: "Charlie" },
	];
}

async function* readStream(
	client: ExampleClient,
	ctx: Cursor,
	_params: Params,
): AsyncIterable<Resumable<TestRow, Cursor>> {
	const items = client.rows.slice(ctx.offset).map((row, i) => ({
		data: new TestRow(row),
		context: { offset: ctx.offset + i + 1 },
	}));
	yield* items;
}

export const ExampleProvider = Provider.withAuthentication("example", {
	credentials: Credentials,
	connect: async (_credentials) => ({
		client: new ExampleClient(),
		disconnect: async () => {},
	}),
});

export const ExampleSource = Stream.createSource("read", ExampleClient, {
	type: TestRow,
	context: Cursor,
	params: Params,
	reader: (client, ctx, params) => readStream(client, ctx, params),
});

export const ExampleTarget = Stream.createTarget("write", ExampleClient, {
	type: TestRow,
	params: Params,
	writer: (_client, _params) => async (_item) => {},
});
