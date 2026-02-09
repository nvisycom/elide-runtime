import { Blob, type Document } from "@nvisy/core";
import { describe, expect, it } from "vitest";
import { plaintextLoader } from "./plaintext.js";

async function collectDocs(iter: AsyncIterable<Document>) {
	const docs = [];
	for await (const doc of iter) {
		docs.push(doc);
	}
	return docs;
}

describe("plaintextLoader", () => {
	it("has id 'plaintext'", () => {
		expect(plaintextLoader.id).toBe("plaintext");
	});

	it("matches .txt extension", () => {
		expect(plaintextLoader.extensions).toContain(".txt");
	});

	it("matches text/plain content type", () => {
		expect(plaintextLoader.contentTypes).toContain("text/plain");
	});

	it("converts utf-8 text blob to document", async () => {
		const blob = new Blob("readme.txt", Buffer.from("Hello, world!"));
		const docs = await collectDocs(
			plaintextLoader.load(blob, { encoding: "utf-8" }),
		);

		expect(docs).toHaveLength(1);
		expect(docs[0]!.content).toBe("Hello, world!");
	});

	it("derives document from blob (sets parentId)", async () => {
		const blob = new Blob("file.txt", Buffer.from("content"));
		const docs = await collectDocs(
			plaintextLoader.load(blob, { encoding: "utf-8" }),
		);

		expect(docs[0]!.parentId).toBe(blob.id);
		expect(docs[0]!.isDerived).toBe(true);
	});

	it("handles empty file", async () => {
		const blob = new Blob("empty.txt", Buffer.alloc(0));
		const docs = await collectDocs(
			plaintextLoader.load(blob, { encoding: "utf-8" }),
		);

		expect(docs).toHaveLength(1);
		expect(docs[0]!.content).toBe("");
	});

	it("handles multiline content", async () => {
		const content = "Line 1\nLine 2\nLine 3";
		const blob = new Blob("multi.txt", Buffer.from(content));
		const docs = await collectDocs(
			plaintextLoader.load(blob, { encoding: "utf-8" }),
		);

		expect(docs[0]!.content).toBe(content);
	});

	it("supports ascii encoding", async () => {
		const blob = new Blob("ascii.txt", Buffer.from("ASCII text", "ascii"));
		const docs = await collectDocs(
			plaintextLoader.load(blob, { encoding: "ascii" }),
		);

		expect(docs[0]!.content).toBe("ASCII text");
	});

	it("supports latin1 encoding", async () => {
		const blob = new Blob("latin.txt", Buffer.from("café", "latin1"));
		const docs = await collectDocs(
			plaintextLoader.load(blob, { encoding: "latin1" }),
		);

		expect(docs[0]!.content).toBe("café");
	});

	it("defaults to utf-8 when encoding not specified", async () => {
		const blob = new Blob("utf8.txt", Buffer.from("Unicode: 你好"));
		const params = plaintextLoader.schema.parse({});
		const docs = await collectDocs(plaintextLoader.load(blob, params));

		expect(docs[0]!.content).toBe("Unicode: 你好");
	});

	it("schema validates encoding enum", () => {
		expect(() =>
			plaintextLoader.schema.parse({ encoding: "invalid" }),
		).toThrow();
	});

	it("schema rejects unknown properties", () => {
		expect(() =>
			plaintextLoader.schema.parse({ encoding: "utf-8", extra: "field" }),
		).toThrow();
	});
});
