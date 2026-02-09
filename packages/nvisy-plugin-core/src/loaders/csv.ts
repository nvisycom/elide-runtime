/**
 * CSV loader.
 *
 * Converts `.csv` and `.tsv` blobs into a single Document.
 * When a header row is present the content is formatted as
 * `"column: value"` blocks separated by blank lines; otherwise
 * raw delimited rows are used.
 *
 * @module
 */

import { type Blob, Document, Loader } from "@nvisy/core";
import { parse } from "csv-parse/sync";
import { z } from "zod";

/** Schema for CSV loader parameters. */
export const csvParamsSchema = z
	.object({
		/** Column delimiter. Defaults to `","`. */
		delimiter: z.string().optional().default(","),
		/** Whether the first row contains column headers. Defaults to `true`. */
		hasHeader: z.boolean().optional().default(true),
		/** Character encoding of the blob data. Defaults to `"utf-8"`. */
		encoding: z
			.enum(["utf-8", "ascii", "latin1", "utf16le"])
			.optional()
			.default("utf-8"),
	})
	.strict();

export type CsvParams = z.infer<typeof csvParamsSchema>;

/**
 * Loader that converts CSV/TSV blobs into a single Document.
 *
 * Header columns are stored as metadata on the Document.
 */
export const csvLoader = Loader.define<CsvParams>("csv", {
	extensions: [".csv", ".tsv"],
	contentTypes: ["text/csv", "text/tab-separated-values"],
	params: csvParamsSchema,
	load: loadCsv,
});

async function* loadCsv(
	blob: Blob,
	params: CsvParams,
): AsyncGenerator<Document> {
	const text = blob.data.toString(params.encoding);
	if (text.trim().length === 0) return;

	const records: string[][] = parse(text, {
		delimiter: params.delimiter,
		relax_column_count: true,
		skip_empty_lines: true,
	});
	if (records.length === 0) return;

	let headers: string[] | null = null;
	let dataRows: string[][] = records;

	if (params.hasHeader) {
		headers = records[0]!;
		dataRows = records.slice(1);
	}

	if (dataRows.length === 0) return;

	const content = headers
		? dataRows
				.map((row) => headers.map((h, j) => `${h}: ${row[j] ?? ""}`).join("\n"))
				.join("\n\n")
		: dataRows.map((row) => row.join(params.delimiter)).join("\n");

	const doc = new Document(content);
	doc.deriveFrom(blob);
	yield doc;
}
