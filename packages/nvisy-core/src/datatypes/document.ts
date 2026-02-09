/**
 * Document data type with optional structured elements.
 *
 * @module
 */

import type { Element } from "../documents/elements.js";
import { Data } from "./data.js";

/** Options for constructing a {@link Document}. */
export interface DocumentOptions {
	/** Document title (e.g. HTML `<title>`, PDF metadata). */
	readonly title?: string;
	/** Pre-extracted structural elements. */
	readonly elements?: readonly Element[];
}

/**
 * A parsed human-readable text representation of a document.
 *
 * Represents extracted text from a partition step — the raw bytes have
 * already been converted into plain text that can be chunked, enriched,
 * or embedded.
 *
 * Structural detail is carried as a flat array of {@link Element}
 * instances. Hierarchy is expressed via `parentId` references and page
 * membership via `pageNumber` on each element.
 *
 * @example
 * ```ts
 * const doc = Document.fromElements([
 *   new Element({ type: "title", text: "Quarterly Report", pageNumber: 1 }),
 *   new Element({ type: "narrative-text", text: "Revenue increased…", pageNumber: 1 }),
 * ]);
 * ```
 */
export class Document extends Data {
	readonly #content: string;
	readonly #title?: string | undefined;
	readonly #elements?: readonly Element[] | undefined;

	constructor(content: string, options?: DocumentOptions) {
		super();
		this.#content = content;
		this.#title = options?.title;
		this.#elements = options?.elements;
	}

	/** Text content of the document. */
	get content(): string {
		return this.#content;
	}

	/** Document title (e.g. HTML `<title>`, PDF metadata). */
	get title(): string | undefined {
		return this.#title;
	}

	/** Unique BCP-47 language tags collected from all elements. */
	get languages(): readonly string[] {
		if (this.#elements == null) return [];
		const uniqueLanguages = new Set<string>();
		for (const element of this.#elements) {
			if (element.languages != null) {
				for (const language of element.languages) {
					uniqueLanguages.add(language);
				}
			}
		}
		return [...uniqueLanguages];
	}

	/** Flat ordered list of structural elements. */
	get elements(): readonly Element[] | undefined {
		return this.#elements;
	}

	/**
	 * Group elements by their 1-based page number.
	 *
	 * Returns a `Map` keyed by page number with each value being the
	 * ordered array of elements on that page. Elements without a
	 * `pageNumber` are collected under key `0`.
	 */
	getElementsByPage(): Map<number, Element[]> {
		const map = new Map<number, Element[]>();
		if (this.#elements == null) return map;
		for (const el of this.#elements) {
			const page = el.pageNumber ?? 0;
			let bucket = map.get(page);
			if (bucket == null) {
				bucket = [];
				map.set(page, bucket);
			}
			bucket.push(el);
		}
		return map;
	}

	/**
	 * Create a Document by deriving `content` from the element texts.
	 *
	 * Element texts are joined with `\n\n` separators.
	 */
	static fromElements(
		elements: readonly Element[],
		options?: Omit<DocumentOptions, "elements">,
	): Document {
		const content = elements.map((el) => el.text).join("\n\n");
		return new Document(content, { ...options, elements });
	}
}
