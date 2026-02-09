import { describe, expect, it } from "vitest";
import { Element } from "../documents/elements.js";
import { Document } from "./document.js";

describe("Document", () => {
	it("stores content and has no elements by default", () => {
		const doc = new Document("hello world");
		expect(doc.content).toBe("hello world");
		expect(doc.elements).toBeUndefined();
	});

	it("constructor accepts elements in options", () => {
		const el = new Element({
			type: "narrative-text",
			text: "Hello",
		});
		const doc = new Document("Hello", { elements: [el] });
		expect(doc.content).toBe("Hello");
		expect(doc.elements).toHaveLength(1);
		expect(doc.elements![0]!.text).toBe("Hello");
	});

	describe("title", () => {
		it("is undefined by default", () => {
			const doc = new Document("text");
			expect(doc.title).toBeUndefined();
		});

		it("is set via constructor options", () => {
			const doc = new Document("text", { title: "Quarterly Report" });
			expect(doc.title).toBe("Quarterly Report");
		});

		it("is preserved by fromElements", () => {
			const el = new Element({
				type: "narrative-text",
				text: "hi",
			});
			const doc = Document.fromElements([el], {
				title: "My Page",
			});
			expect(doc.title).toBe("My Page");
		});
	});

	describe("languages", () => {
		it("is empty when there are no elements", () => {
			const doc = new Document("text");
			expect(doc.languages).toEqual([]);
		});

		it("is empty when no elements have languages", () => {
			const doc = new Document("text", {
				elements: [
					new Element({
						type: "narrative-text",
						text: "hello",
					}),
				],
			});
			expect(doc.languages).toEqual([]);
		});

		it("collects unique languages from all elements", () => {
			const doc = new Document("text", {
				elements: [
					new Element({
						type: "narrative-text",
						text: "hello",
						languages: ["en"],
					}),
					new Element({
						type: "narrative-text",
						text: "hallo",
						languages: ["de", "en"],
					}),
					new Element({
						type: "narrative-text",
						text: "bonjour",
						languages: ["fr"],
					}),
				],
			});
			expect(doc.languages).toEqual(["en", "de", "fr"]);
		});

		it("skips elements without languages", () => {
			const doc = new Document("text", {
				elements: [
					new Element({
						type: "narrative-text",
						text: "no lang",
					}),
					new Element({
						type: "narrative-text",
						text: "has lang",
						languages: ["es"],
					}),
				],
			});
			expect(doc.languages).toEqual(["es"]);
		});
	});

	describe("fromElements", () => {
		it("derives content from element texts joined with \\n\\n", () => {
			const elements = [
				new Element({
					type: "title",
					text: "Title",
					level: 1,
				}),
				new Element({
					type: "narrative-text",
					text: "First paragraph.",
				}),
				new Element({
					type: "narrative-text",
					text: "Second paragraph.",
				}),
			];
			const doc = Document.fromElements(elements);
			expect(doc.content).toBe(
				"Title\n\nFirst paragraph.\n\nSecond paragraph.",
			);
			expect(doc.elements).toHaveLength(3);
		});

		it("produces empty content from empty elements array", () => {
			const doc = Document.fromElements([]);
			expect(doc.content).toBe("");
			expect(doc.elements).toEqual([]);
		});
	});

	describe("getElementsByPage", () => {
		it("returns empty map when there are no elements", () => {
			const doc = new Document("text");
			expect(doc.getElementsByPage().size).toBe(0);
		});

		it("groups elements by pageNumber", () => {
			const doc = new Document("text", {
				elements: [
					new Element({ type: "title", text: "Title", pageNumber: 1 }),
					new Element({ type: "narrative-text", text: "p1", pageNumber: 1 }),
					new Element({ type: "narrative-text", text: "p2", pageNumber: 2 }),
				],
			});
			const pages = doc.getElementsByPage();
			expect(pages.size).toBe(2);
			expect(pages.get(1)).toHaveLength(2);
			expect(pages.get(2)).toHaveLength(1);
			expect(pages.get(2)![0].text).toBe("p2");
		});

		it("collects elements without pageNumber under key 0", () => {
			const doc = new Document("text", {
				elements: [
					new Element({ type: "title", text: "Title" }),
					new Element({ type: "narrative-text", text: "p1", pageNumber: 1 }),
				],
			});
			const pages = doc.getElementsByPage();
			expect(pages.get(0)).toHaveLength(1);
			expect(pages.get(0)![0].text).toBe("Title");
			expect(pages.get(1)).toHaveLength(1);
		});

		it("preserves element order within each page", () => {
			const doc = new Document("text", {
				elements: [
					new Element({ type: "title", text: "A", pageNumber: 1 }),
					new Element({ type: "narrative-text", text: "B", pageNumber: 2 }),
					new Element({ type: "narrative-text", text: "C", pageNumber: 1 }),
				],
			});
			const page1 = doc.getElementsByPage().get(1)!;
			expect(page1.map((e) => e.text)).toEqual(["A", "C"]);
		});
	});

	describe("Element", () => {
		it("auto-generates a unique id", () => {
			const a = new Element({
				type: "narrative-text",
				text: "a",
			});
			const b = new Element({
				type: "narrative-text",
				text: "b",
			});
			expect(a.id).toBeTruthy();
			expect(b.id).toBeTruthy();
			expect(a.id).not.toBe(b.id);
		});

		it("carries parentId for hierarchy", () => {
			const table = new Element({ type: "table", text: "" });
			const child = new Element({
				type: "narrative-text",
				text: "Revenue",
				parentId: table.id,
			});
			expect(child.parentId).toBe(table.id);
		});

		it("carries pageNumber", () => {
			const el = new Element({
				type: "title",
				text: "Intro",
				pageNumber: 2,
			});
			expect(el.pageNumber).toBe(2);
		});

		it("carries optional enrichment fields", () => {
			const el = new Element({
				type: "table",
				text: "A | B",
				languages: ["en"],
				provenance: { confidence: 0.95, isContinuation: false },
			});
			expect(el.provenance?.confidence).toBe(0.95);
			expect(el.languages).toEqual(["en"]);
			expect(el.provenance?.isContinuation).toBe(false);
		});

		it("accepts various element types", () => {
			const elements = [
				new Element({ type: "formula", text: "E = mc²" }),
				new Element({ type: "list-item", text: "First item" }),
				new Element({ type: "page-break", text: "" }),
			];
			expect(elements.map((e) => e.type)).toEqual([
				"formula",
				"list-item",
				"page-break",
			]);
		});
	});
});
