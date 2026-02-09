import { Blob, type Document } from "@nvisy/core";
import { describe, expect, it } from "vitest";
import { csvLoader } from "./csv.js";

async function collectDocs(iter: AsyncIterable<Document>) {
	const docs = [];
	for await (const doc of iter) {
		docs.push(doc);
	}
	return docs;
}

describe("csvLoader", () => {
	it("has id 'csv'", () => {
		expect(csvLoader.id).toBe("csv");
	});

	it("matches .csv and .tsv extensions", () => {
		expect(csvLoader.extensions).toContain(".csv");
		expect(csvLoader.extensions).toContain(".tsv");
	});

	it("matches text/csv content type", () => {
		expect(csvLoader.contentTypes).toContain("text/csv");
	});

	it("parses CSV with headers into a single document", async () => {
		const csv = "name,age\nAlice,30\nBob,25";
		const blob = new Blob("data.csv", Buffer.from(csv));
		const docs = await collectDocs(
			csvLoader.load(blob, {
				delimiter: ",",
				hasHeader: true,
				encoding: "utf-8",
			}),
		);

		expect(docs).toHaveLength(1);
		expect(docs[0]!.content).toBe("name: Alice\nage: 30\n\nname: Bob\nage: 25");
	});

	it("parses CSV without headers", async () => {
		const csv = "Alice,30\nBob,25";
		const blob = new Blob("data.csv", Buffer.from(csv));
		const docs = await collectDocs(
			csvLoader.load(blob, {
				delimiter: ",",
				hasHeader: false,
				encoding: "utf-8",
			}),
		);

		expect(docs).toHaveLength(1);
		expect(docs[0]!.content).toBe("Alice,30\nBob,25");
	});

	it("supports tab delimiter for TSV", async () => {
		const tsv = "name\tage\nAlice\t30";
		const blob = new Blob("data.tsv", Buffer.from(tsv));
		const docs = await collectDocs(
			csvLoader.load(blob, {
				delimiter: "\t",
				hasHeader: true,
				encoding: "utf-8",
			}),
		);

		expect(docs).toHaveLength(1);
		expect(docs[0]!.content).toBe("name: Alice\nage: 30");
	});

	it("handles quoted fields with commas", async () => {
		const csv = 'name,address\nAlice,"123 Main St, Apt 4"';
		const blob = new Blob("data.csv", Buffer.from(csv));
		const docs = await collectDocs(
			csvLoader.load(blob, {
				delimiter: ",",
				hasHeader: true,
				encoding: "utf-8",
			}),
		);

		expect(docs[0]!.content).toContain("address: 123 Main St, Apt 4");
	});

	it("handles escaped quotes in fields", async () => {
		const csv = 'name,note\nAlice,"She said ""hello"""';
		const blob = new Blob("data.csv", Buffer.from(csv));
		const docs = await collectDocs(
			csvLoader.load(blob, {
				delimiter: ",",
				hasHeader: true,
				encoding: "utf-8",
			}),
		);

		expect(docs[0]!.content).toContain('note: She said "hello"');
	});

	it("derives document from blob", async () => {
		const csv = "a\n1\n2";
		const blob = new Blob("data.csv", Buffer.from(csv));
		const docs = await collectDocs(
			csvLoader.load(blob, {
				delimiter: ",",
				hasHeader: true,
				encoding: "utf-8",
			}),
		);

		expect(docs[0]!.parentId).toBe(blob.id);
	});

	it("handles empty file", async () => {
		const blob = new Blob("empty.csv", Buffer.alloc(0));
		const docs = await collectDocs(
			csvLoader.load(blob, {
				delimiter: ",",
				hasHeader: true,
				encoding: "utf-8",
			}),
		);

		expect(docs).toHaveLength(0);
	});

	it("handles header-only file", async () => {
		const csv = "name,age";
		const blob = new Blob("header.csv", Buffer.from(csv));
		const docs = await collectDocs(
			csvLoader.load(blob, {
				delimiter: ",",
				hasHeader: true,
				encoding: "utf-8",
			}),
		);

		expect(docs).toHaveLength(0);
	});

	it("handles CRLF line endings", async () => {
		const csv = "name,age\r\nAlice,30\r\nBob,25";
		const blob = new Blob("data.csv", Buffer.from(csv));
		const docs = await collectDocs(
			csvLoader.load(blob, {
				delimiter: ",",
				hasHeader: true,
				encoding: "utf-8",
			}),
		);

		expect(docs).toHaveLength(1);
		expect(docs[0]!.content).toBe("name: Alice\nage: 30\n\nname: Bob\nage: 25");
	});

	it("uses defaults for optional params", async () => {
		const csv = "a,b\n1,2";
		const blob = new Blob("data.csv", Buffer.from(csv));
		const params = csvLoader.schema.parse({});
		const docs = await collectDocs(csvLoader.load(blob, params));

		expect(docs).toHaveLength(1);
		expect(docs[0]!.content).toBe("a: 1\nb: 2");
	});

	it("schema rejects unknown properties", () => {
		expect(() => csvLoader.schema.parse({ extra: "field" })).toThrow();
	});
});
