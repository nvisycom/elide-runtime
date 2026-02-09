import { getLogger } from "@logtape/logtape";
import { Embedding, RuntimeError, Stream } from "@nvisy/core";
import { z } from "zod";
import { VectorClient } from "../providers/client.js";

const logger = getLogger(["nvisy", "vector"]);

/**
 * Per-node parameters for the vector upsert stream.
 */
export const UpsertParams = z.object({});
export type UpsertParams = z.infer<typeof UpsertParams>;

/**
 * Target stream that upserts each {@link Embedding} into the vector store
 * via the provider client's `upsert` method.
 */
export const upsert = Stream.createTarget("upsert", VectorClient, {
	type: Embedding,
	params: UpsertParams,
	writer:
		(client: VectorClient, _params: UpsertParams) =>
		async (item: Embedding) => {
			try {
				await client.upsert([
					{
						id: item.id,
						vector: item.vector,
						metadata: item.metadata ?? undefined,
					},
				]);
				logger.debug("Upserted vector {id} ({dims} dims)", {
					id: item.id,
					dims: item.dimensions,
				});
			} catch (error) {
				logger.error("Upsert failed for {id}: {error}", {
					id: item.id,
					error: error instanceof Error ? error.message : String(error),
				});
				throw RuntimeError.wrap(error, { source: "vector/upsert" });
			}
		},
});
