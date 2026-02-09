import { getLogger } from "@logtape/logtape";
import { Pinecone } from "@pinecone-database/pinecone";
import { z } from "zod";
import {
	makeVectorProvider,
	type UpsertVector,
	VectorClient,
	VectorProvider,
} from "./client.js";

const logger = getLogger(["nvisy", "vector"]);

/**
 * Credentials for connecting to Pinecone.
 */
export const PineconeCredentials = z.object({
	/** Pinecone API key. */
	apiKey: z.string(),
	/** Name of the Pinecone index. */
	indexName: z.string(),
});
export type PineconeCredentials = z.infer<typeof PineconeCredentials>;

class PineconeVectorClient extends VectorClient {
	readonly #index: ReturnType<Pinecone["index"]>;

	constructor(index: ReturnType<Pinecone["index"]>) {
		super();
		this.#index = index;
	}

	async upsert(vectors: UpsertVector[]): Promise<void> {
		await this.#index.upsert({
			records: vectors.map((v) => ({
				id: v.id,
				values: [...v.vector],
				metadata: v.metadata as Record<string, string>,
			})),
		});
	}
}

/** Pinecone vector database provider. */
export const pinecone = makeVectorProvider(
	"pinecone",
	PineconeCredentials,
	async (creds) => {
		logger.debug("Connecting to Pinecone index {indexName}", {
			indexName: creds.indexName,
		});

		const client = new Pinecone({ apiKey: creds.apiKey });
		const index = client.index(creds.indexName);

		return new VectorProvider(new PineconeVectorClient(index), "pinecone");
	},
);
