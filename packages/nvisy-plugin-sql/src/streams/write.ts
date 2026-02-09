import { getLogger } from "@logtape/logtape";
import { RuntimeError, Stream } from "@nvisy/core";
import { Row } from "../datatypes/index.js";
import { KyselyClient } from "../providers/client.js";
import { SqlParams } from "./schemas.js";

const logger = getLogger(["nvisy", "sql"]);

/**
 * Per-item insert target stream.
 *
 * Extracts the column map from each {@link Row} and writes it via
 * a Kysely INSERT. Each element piped through the writer triggers
 * an individual INSERT statement.
 */
export const write = Stream.createTarget("write", KyselyClient, {
	type: Row,
	params: SqlParams,
	writer: (client, params) => async (item: Row) => {
		const record = item.columns as Record<string, unknown>;
		if (Object.keys(record).length === 0) return;

		try {
			await client.db.insertInto(params.table).values(record).execute();
			logger.debug("Inserted row into {table}", { table: params.table });
		} catch (error) {
			logger.error("Write failed on {table}: {error}", {
				table: params.table,
				error: error instanceof Error ? error.message : String(error),
			});
			throw RuntimeError.wrap(error, { source: "sql/write" });
		}
	},
});
