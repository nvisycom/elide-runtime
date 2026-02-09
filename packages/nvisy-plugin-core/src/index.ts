import {
	Blob,
	Chunk,
	Datatype,
	Document,
	Embedding,
	Plugin,
} from "@nvisy/core";
import { chunkSimple, partition } from "./actions/index.js";
import { csvLoader, jsonLoader, plaintextLoader } from "./loaders/index.js";

export const corePlugin = Plugin.define("core")
	.withDatatypes(
		Datatype.define("document", Document),
		Datatype.define("blob", Blob),
		Datatype.define("chunk", Chunk),
		Datatype.define("embedding", Embedding),
	)
	.withActions(chunkSimple, partition)
	.withLoaders(plaintextLoader, csvLoader, jsonLoader);

export type {
	CharacterStrategyParams,
	PageStrategyParams,
	SectionStrategyParams,
} from "./actions/chunk.js";
export { chunkSimple, partition } from "./actions/index.js";
export type {
	AutoStrategyParams,
	RuleStrategyParams,
} from "./actions/partition.js";
export type { CsvParams } from "./loaders/csv.js";
export { csvLoader, csvParamsSchema } from "./loaders/csv.js";
export type { JsonParams } from "./loaders/json.js";
export { jsonLoader, jsonParamsSchema } from "./loaders/json.js";
export type { PlaintextParams } from "./loaders/plaintext.js";
export { plaintextLoader, plaintextParamsSchema } from "./loaders/plaintext.js";
export type {
	DelimiterSplitOptions,
	RegexSplitOptions,
} from "./splitter/index.js";
export { splitByDelimiter, splitByRegex } from "./splitter/index.js";
