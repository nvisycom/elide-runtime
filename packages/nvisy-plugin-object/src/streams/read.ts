import { getLogger } from "@logtape/logtape";
import { Blob, type Resumable, RuntimeError, Stream } from "@nvisy/core";
import { z } from "zod";
import { ObjectStoreClient } from "../providers/client.js";

const logger = getLogger(["nvisy", "object"]);

/**
 * Per-node parameters for the object-store read stream.
 */
export const ObjectParams = z.object({
	/** Key prefix to list objects under (e.g. `"uploads/2024/"`). */
	prefix: z.string().default(""),
	/** Maximum keys to fetch per list page. */
	batchSize: z.number().default(100),
});
export type ObjectParams = z.infer<typeof ObjectParams>;

/**
 * Keyset pagination cursor for resumable object reads.
 *
 * `lastKey` is `null` on the very first page.
 */
export const ObjectCursor = z.object({
	/** The last key successfully yielded, or `null` before the first page. */
	lastKey: z.string().nullable().default(null),
});
export type ObjectCursor = z.infer<typeof ObjectCursor>;

/**
 * Source stream that lists objects under a prefix and yields each as
 * a {@link Blob}. Pagination uses the last-key cursor from the store's
 * list API.
 */
export const read = Stream.createSource("read", ObjectStoreClient, {
	type: Blob,
	context: ObjectCursor,
	params: ObjectParams,
	reader: (client, cursor, params) => readStream(client, cursor, params),
});

async function* readStream(
	client: ObjectStoreClient,
	cursor: ObjectCursor,
	params: ObjectParams,
): AsyncIterable<Resumable<Blob, ObjectCursor>> {
	const { prefix, batchSize } = params;

	logger.debug("Read stream opened on prefix {prefix}", { prefix, batchSize });

	let nextCursor: string | undefined = cursor.lastKey ?? undefined;
	let totalObjects = 0;

	while (true) {
		let keys: readonly string[];
		let pageCursor: string | undefined;

		try {
			const result = await client.list(prefix, nextCursor);
			keys = result.keys;
			pageCursor = result.nextCursor;
			logger.debug("List returned {count} keys", { count: keys.length });
		} catch (error) {
			logger.error("List failed for prefix {prefix}: {error}", {
				prefix,
				error: error instanceof Error ? error.message : String(error),
			});
			throw RuntimeError.wrap(error, { source: "object/read" });
		}

		for (const key of keys) {
			try {
				const { data, contentType } = await client.get(key);
				totalObjects++;
				yield {
					data: new Blob(key, data, { contentType }),
					context: { lastKey: key } as ObjectCursor,
				};
			} catch (error) {
				logger.error("Get failed for key {key}: {error}", {
					key,
					error: error instanceof Error ? error.message : String(error),
				});
				throw RuntimeError.wrap(error, { source: "object/read" });
			}
		}

		if (keys.length < batchSize || !pageCursor) break;
		nextCursor = pageCursor;
	}

	logger.debug("Read stream closed, {totalObjects} objects yielded", {
		totalObjects,
	});
}
