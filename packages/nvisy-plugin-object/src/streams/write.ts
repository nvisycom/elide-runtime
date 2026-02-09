import { getLogger } from "@logtape/logtape";
import { Blob, RuntimeError, Stream } from "@nvisy/core";
import { z } from "zod";
import { ObjectStoreClient } from "../providers/client.js";

const logger = getLogger(["nvisy", "object"]);

/**
 * Per-node parameters for the object-store write stream.
 */
export const WriteParams = z.object({
	/** Key prefix to prepend to each blob path on write. */
	prefix: z.string().default(""),
});
export type WriteParams = z.infer<typeof WriteParams>;

/**
 * Target stream that writes each {@link Blob} to the object store
 * via the provider client's `put` method.
 */
export const write = Stream.createTarget("write", ObjectStoreClient, {
	type: Blob,
	params: WriteParams,
	writer: (client, params) => async (item: Blob) => {
		const key = params.prefix ? `${params.prefix}${item.path}` : item.path;
		try {
			await client.put(key, item.data, item.provided.mime);
			logger.debug("Put object {key} ({size} bytes)", {
				key,
				size: item.size,
			});
		} catch (error) {
			logger.error("Put failed for {key}: {error}", {
				key,
				error: error instanceof Error ? error.message : String(error),
			});
			throw RuntimeError.wrap(error, { source: "object/write" });
		}
	},
});
