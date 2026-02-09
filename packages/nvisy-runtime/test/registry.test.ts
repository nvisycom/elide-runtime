import { Datatype, Document, Plugin, ValidationError } from "@nvisy/core";
import { describe, expect, it } from "vitest";
import { Registry } from "../src/registry.js";
import {
	makeTestRegistry,
	noopAction,
	testPlugin,
	testProvider,
	testSourceStream,
	testTargetStream,
} from "./fixtures.js";

describe("Registry", () => {
	it("loads a plugin and resolves its entries by qualified name", () => {
		const registry = makeTestRegistry();

		expect(registry.getAction("test/noop")).toBe(noopAction);
		expect(registry.getProvider("test/testdb")).toBe(testProvider);
		expect(registry.getStream("test/read")).toBe(testSourceStream);
		expect(registry.getStream("test/write")).toBe(testTargetStream);
	});

	it("find* returns undefined for missing entries", () => {
		const registry = makeTestRegistry();

		expect(registry.findAction("missing/action")).toBeUndefined();
		expect(registry.findProvider("missing/provider")).toBeUndefined();
		expect(registry.findStream("missing/stream")).toBeUndefined();
		expect(registry.findDataType("missing/type")).toBeUndefined();
	});

	it("get* throws ValidationError for missing entries", () => {
		const registry = makeTestRegistry();

		expect(() => registry.getAction("missing/action")).toThrow(ValidationError);
		expect(() => registry.getProvider("missing/provider")).toThrow(
			ValidationError,
		);
		expect(() => registry.getStream("missing/stream")).toThrow(ValidationError);
		expect(() => registry.getDataType("missing/type")).toThrow(ValidationError);
	});

	it("rejects loading the same plugin twice", () => {
		const registry = makeTestRegistry();

		expect(() => registry.load(testPlugin)).toThrow("Plugin already loaded");
	});

	it("loads datatypes and resolves them", () => {
		const registry = new Registry();
		const plugin = Plugin.define("dt").withDatatypes(
			Datatype.define("document", Document),
		);
		registry.load(plugin);

		const entry = registry.getDataType("dt/document");
		expect(entry.id).toBe("document");
		expect(entry.dataClass).toBe(Document);
	});

	it("schema snapshot includes actions and providers", () => {
		const registry = makeTestRegistry();
		const schema = registry.schema;

		expect(schema.actions).toHaveLength(1);
		expect(schema.actions[0]!.name).toBe("test/noop");
		expect(schema.providers).toHaveLength(1);
		expect(schema.providers[0]!.name).toBe("test/testdb");
		expect(schema.streams).toBe(2);
		expect(schema.loaders).toBe(0);
		expect(schema.datatypes).toBe(0);
	});

	it("schema snapshot is empty for a fresh registry", () => {
		const registry = new Registry();
		const schema = registry.schema;

		expect(schema.actions).toHaveLength(0);
		expect(schema.providers).toHaveLength(0);
		expect(schema.streams).toBe(0);
		expect(schema.loaders).toBe(0);
		expect(schema.datatypes).toBe(0);
	});
});
