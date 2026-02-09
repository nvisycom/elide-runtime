import { Blob, type Document } from "@nvisy/core";
import { describe, expect, it } from "vitest";
import { jsonLoader } from "./json.js";

async function collectDocs(iter: AsyncIterable<Document>) {
	const docs = [];
	for await (const doc of iter) {
		docs.push(doc);
	}
	return docs;
}

describe("jsonLoader", () => {
	it("has id 'json'", () => {
		expect(jsonLoader.id).toBe("json");
	});

	it("matches .json, .jsonl, and .ndjson extensions", () => {
		expect(jsonLoader.extensions).toContain(".json");
		expect(jsonLoader.extensions).toContain(".jsonl");
		expect(jsonLoader.extensions).toContain(".ndjson");
	});

	it("matches application/json content type", () => {
		expect(jsonLoader.contentTypes).toContain("application/json");
	});

	describe("JSON files", () => {
		it("creates one document from a JSON object", async () => {
			const json = JSON.stringify({ name: "Alice", age: 30 });
			const blob = new Blob("data.json", Buffer.from(json));
			const docs = await collectDocs(
				jsonLoader.load(blob, { encoding: "utf-8" }),
			);

			expect(docs).toHaveLength(1);
		});

		it("promotes scalar fields to metadata", async () => {
			const json = JSON.stringify({ name: "Alice", age: 30, active: true });
			const blob = new Blob("data.json", Buffer.from(json));
			const docs = await collectDocs(
				jsonLoader.load(blob, { encoding: "utf-8" }),
			);

			expect(docs[0]!.metadata).toMatchObject({
				name: "Alice",
				age: 30,
				active: true,
			});
		});

		it("creates one document from a JSON array", async () => {
			const json = JSON.stringify([
				{ id: 1, text: "first" },
				{ id: 2, text: "second" },
			]);
			const blob = new Blob("data.json", Buffer.from(json));
			const docs = await collectDocs(
				jsonLoader.load(blob, { encoding: "utf-8" }),
			);

			expect(docs).toHaveLength(1);
			expect(docs[0]!.content).toBe(
				JSON.stringify(
					[
						{ id: 1, text: "first" },
						{ id: 2, text: "second" },
					],
					null,
					2,
				),
			);
		});

		it("handles string JSON values", async () => {
			const json = JSON.stringify("just a string");
			const blob = new Blob("data.json", Buffer.from(json));
			const docs = await collectDocs(
				jsonLoader.load(blob, { encoding: "utf-8" }),
			);

			expect(docs).toHaveLength(1);
			expect(docs[0]!.content).toBe("just a string");
		});

		it("pretty-prints object content", async () => {
			const obj = { key: "value" };
			const blob = new Blob("data.json", Buffer.from(JSON.stringify(obj)));
			const docs = await collectDocs(
				jsonLoader.load(blob, { encoding: "utf-8" }),
			);

			expect(docs[0]!.content).toBe(JSON.stringify(obj, null, 2));
		});

		it("derives document from blob", async () => {
			const json = JSON.stringify({ a: 1 });
			const blob = new Blob("data.json", Buffer.from(json));
			const docs = await collectDocs(
				jsonLoader.load(blob, { encoding: "utf-8" }),
			);

			expect(docs[0]!.parentId).toBe(blob.id);
		});
	});

	describe("JSONL files", () => {
		it("creates one document from JSONL", async () => {
			const jsonl = '{"id":1}\n{"id":2}\n{"id":3}';
			const blob = new Blob("data.jsonl", Buffer.from(jsonl));
			const docs = await collectDocs(
				jsonLoader.load(blob, { encoding: "utf-8" }),
			);

			expect(docs).toHaveLength(1);
			expect(docs[0]!.content).toBe(
				JSON.stringify([{ id: 1 }, { id: 2 }, { id: 3 }], null, 2),
			);
		});

		it("skips empty lines", async () => {
			const jsonl = '{"a":1}\n\n{"b":2}\n';
			const blob = new Blob("data.jsonl", Buffer.from(jsonl));
			const docs = await collectDocs(
				jsonLoader.load(blob, { encoding: "utf-8" }),
			);

			expect(docs).toHaveLength(1);
			expect(docs[0]!.content).toBe(
				JSON.stringify([{ a: 1 }, { b: 2 }], null, 2),
			);
		});

		it("handles .ndjson extension", async () => {
			const ndjson = '{"x":1}\n{"x":2}';
			const blob = new Blob("data.ndjson", Buffer.from(ndjson));
			const docs = await collectDocs(
				jsonLoader.load(blob, { encoding: "utf-8" }),
			);

			expect(docs).toHaveLength(1);
		});

		it("derives document from blob", async () => {
			const jsonl = '{"a":1}\n{"b":2}';
			const blob = new Blob("data.jsonl", Buffer.from(jsonl));
			const docs = await collectDocs(
				jsonLoader.load(blob, { encoding: "utf-8" }),
			);

			expect(docs[0]!.parentId).toBe(blob.id);
		});
	});

	it("uses defaults for optional params", async () => {
		const json = JSON.stringify({ hello: "world" });
		const blob = new Blob("data.json", Buffer.from(json));
		const params = jsonLoader.schema.parse({});
		const docs = await collectDocs(jsonLoader.load(blob, params));

		expect(docs).toHaveLength(1);
	});

	it("schema rejects unknown properties", () => {
		expect(() => jsonLoader.schema.parse({ extra: "field" })).toThrow();
	});
});
