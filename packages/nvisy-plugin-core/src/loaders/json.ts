/**
 * JSON / JSON Lines loader.
 *
 * Converts `.json`, `.jsonl`, and `.ndjson` blobs into a single
 * Document whose content is the pretty-printed JSON text.
 * For JSONL/NDJSON files the lines are collected into an array first.
 *
 * @module
 */

import { type Blob, Document, Loader } from "@nvisy/core";
import { z } from "zod";

/** Schema for JSON loader parameters. */
export const jsonParamsSchema = z
	.object({
		/** Character encoding of the blob data. Defaults to `"utf-8"`. */
		encoding: z
			.enum(["utf-8", "ascii", "latin1", "utf16le"])
			.optional()
			.default("utf-8"),
	})
	.strict();

export type JsonParams = z.infer<typeof jsonParamsSchema>;

/**
 * Loader that converts JSON / JSONL blobs into a single Document.
 *
 * Scalar object fields are promoted to metadata.
 */
export const jsonLoader = Loader.define<JsonParams>("json", {
	extensions: [".json", ".jsonl", ".ndjson"],
	contentTypes: ["application/json", "application/x-ndjson"],
	params: jsonParamsSchema,
	load: loadJson,
});

async function* loadJson(
	blob: Blob,
	params: JsonParams,
): AsyncGenerator<Document> {
	const text = blob.data.toString(params.encoding);
	const isJsonLines =
		blob.path.endsWith(".jsonl") || blob.path.endsWith(".ndjson");

	const parsed: unknown = isJsonLines ? parseJsonLines(text) : JSON.parse(text);
	const content =
		typeof parsed === "string" ? parsed : JSON.stringify(parsed, null, 2);

	const doc = new Document(content);
	doc.deriveFrom(blob);

	if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
		const metadata: Record<string, string | number | boolean> = {};
		for (const [k, v] of Object.entries(parsed)) {
			if (
				typeof v === "string" ||
				typeof v === "number" ||
				typeof v === "boolean"
			) {
				metadata[k] = v;
			}
		}
		if (Object.keys(metadata).length > 0) {
			doc.withMetadata(metadata);
		}
	}

	yield doc;
}

/** Parse newline-delimited JSON into an array of values. */
function parseJsonLines(text: string): unknown[] {
	const results: unknown[] = [];
	for (const line of text.split(/\r?\n/)) {
		const trimmed = line.trim();
		if (trimmed.length === 0) continue;
		results.push(JSON.parse(trimmed));
	}
	return results;
}
