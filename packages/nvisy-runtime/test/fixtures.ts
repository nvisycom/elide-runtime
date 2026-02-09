import type { JsonValue, Resumable } from "@nvisy/core";
import { Action, Data, Plugin, Provider, Stream } from "@nvisy/core";
import { z } from "zod";
import type { Connections } from "../src/engine/connections.js";
import { Engine } from "../src/engine/engine.js";
import { Registry } from "../src/registry.js";

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

export const GRAPH_ID = "00000000-0000-4000-8000-000000000000";
export const SOURCE_ID = "00000000-0000-4000-8000-000000000001";
export const ACTION_ID = "00000000-0000-4000-8000-000000000002";
export const TARGET_ID = "00000000-0000-4000-8000-000000000003";
export const EXTRA_ID = "00000000-0000-4000-8000-000000000004";
export const CRED_ID = "00000000-0000-4000-8000-0000000000c0";

const NoopParams = z.object({});

export const noopAction = Action.withoutClient("noop", {
	types: [TestRow],
	params: NoopParams,
	transform: (stream, _params) => stream,
});

const TestCredentials = z.object({ host: z.string() });

class TestClient {}

export const testProvider = Provider.withAuthentication("testdb", {
	credentials: TestCredentials,
	connect: async (_creds) => ({
		client: new TestClient(),
	}),
});

const TestContext = z.object({
	cursor: z.string().nullable().default(null),
});

const TestParams = z.record(z.string(), z.unknown());

/**
 * Items produced by the mock source stream.
 * Exposed so tests can assert on them.
 */
export const sourceEntries: TestRow[] = [
	new TestRow({ name: "Alice", age: 30 }),
	new TestRow({ name: "Bob", age: 25 }),
	new TestRow({ name: "Carol", age: 35 }),
];

export const testSourceStream = Stream.createSource("read", TestClient, {
	type: TestRow,
	context: TestContext,
	params: TestParams,
	reader: async function* (_client, _ctx, _params) {
		for (const row of sourceEntries) {
			yield { data: row, context: { cursor: row.id } } as Resumable<
				TestRow,
				z.infer<typeof TestContext>
			>;
		}
	},
});

/**
 * Items written to the mock target stream.
 * Tests can inspect this array after execution.
 */
export const writtenItems: Data[] = [];

export const testTargetStream = Stream.createTarget("write", TestClient, {
	type: TestRow,
	params: TestParams,
	writer: (_client, _params) => {
		return async (item: TestRow) => {
			writtenItems.push(item);
		};
	},
});

export const testPlugin = Plugin.define("test")
	.withActions(noopAction)
	.withProviders(testProvider)
	.withStreams(testSourceStream, testTargetStream);

/**
 * Create a Registry pre-loaded with the test plugin.
 */
export function makeTestRegistry(): Registry {
	const registry = new Registry();
	registry.load(testPlugin);
	return registry;
}

/**
 * Create an Engine pre-loaded with the test plugin.
 */
export function makeTestEngine(): Engine {
	return new Engine().register(testPlugin);
}

/**
 * Default credential map matching the test provider's schema.
 */
export function testCredentials() {
	return { [CRED_ID]: { host: "localhost" } };
}

/**
 * Default connections map matching the test provider's schema.
 */
export function testConnections(): Connections {
	return {
		[CRED_ID]: {
			type: "testdb",
			credentials: { host: "localhost" },
			context: {},
		},
	};
}

export function linearGraph() {
	return {
		id: GRAPH_ID,
		nodes: [
			{
				id: SOURCE_ID,
				type: "source" as const,
				provider: "test/testdb",
				stream: "test/read",
				connection: CRED_ID,
				params: { table: "users" },
			},
			{
				id: ACTION_ID,
				type: "action" as const,
				action: "test/noop",
				params: {},
			},
			{
				id: TARGET_ID,
				type: "target" as const,
				provider: "test/testdb",
				stream: "test/write",
				connection: CRED_ID,
				params: { table: "output" },
			},
		],
		edges: [
			{ from: SOURCE_ID, to: ACTION_ID },
			{ from: ACTION_ID, to: TARGET_ID },
		],
	};
}

export function isolatedNodesGraph() {
	return {
		id: GRAPH_ID,
		nodes: [
			{
				id: SOURCE_ID,
				type: "source" as const,
				provider: "test/testdb",
				stream: "test/read",
				connection: CRED_ID,
				params: { table: "users" },
			},
			{
				id: ACTION_ID,
				type: "action" as const,
				action: "test/noop",
				params: {},
			},
		],
		edges: [],
	};
}

export function diamondGraph() {
	return {
		id: GRAPH_ID,
		nodes: [
			{
				id: SOURCE_ID,
				type: "source" as const,
				provider: "test/testdb",
				stream: "test/read",
				connection: CRED_ID,
				params: { table: "users" },
			},
			{
				id: ACTION_ID,
				type: "action" as const,
				action: "test/noop",
				params: {},
			},
			{
				id: EXTRA_ID,
				type: "action" as const,
				action: "test/noop",
				params: {},
			},
			{
				id: TARGET_ID,
				type: "target" as const,
				provider: "test/testdb",
				stream: "test/write",
				connection: CRED_ID,
				params: { table: "output" },
			},
		],
		edges: [
			{ from: SOURCE_ID, to: ACTION_ID },
			{ from: SOURCE_ID, to: EXTRA_ID },
			{ from: ACTION_ID, to: TARGET_ID },
			{ from: EXTRA_ID, to: TARGET_ID },
		],
	};
}
