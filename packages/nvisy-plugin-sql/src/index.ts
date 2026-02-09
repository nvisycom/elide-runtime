/**
 * @module @nvisy/plugin-sql
 *
 * SQL provider plugin for the Nvisy runtime.
 *
 * Exposes Postgres, MySQL, and MSSQL providers (client lifecycle only),
 * read/write streams (keyset-paginated source + batch-insert sink), and
 * row-level transform actions (filter, project, rename, coerce).
 *
 * @example
 * ```ts
 * import { sqlPlugin } from "@nvisy/plugin-sql";
 *
 * // Register with the runtime
 * runtime.register(sqlPlugin);
 * ```
 */

import { Datatype, Plugin } from "@nvisy/core";
import { coerce, filter, project, rename } from "./actions/index.js";
import { Row } from "./datatypes/index.js";
import { mssql, mysql, postgres } from "./providers/index.js";
import { read, write } from "./streams/index.js";

/** The SQL plugin: register this with the runtime to enable all SQL providers, streams, and actions. */
export const sqlPlugin = Plugin.define("sql")
	.withProviders(postgres, mysql, mssql)
	.withStreams(read, write)
	.withActions(filter, project, rename, coerce)
	.withDatatypes(Datatype.define("row", Row));

export { Row } from "./datatypes/index.js";
