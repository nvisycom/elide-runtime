/**
 * @module datatypes
 *
 * Base data model and built-in types for the Nvisy pipeline.
 */

export type { BlobOptions, Filetype } from "./blob.js";
export { Blob } from "./blob.js";
export type { ChunkOptions } from "./chunk.js";
export { Chunk } from "./chunk.js";
export { Data } from "./data.js";
export type { DocumentOptions } from "./document.js";
export { Document } from "./document.js";
export { Embedding } from "./embedding.js";

import type { ClassRef } from "../types.js";
import type { Data } from "./data.js";

/**
 * A custom data type registered by a plugin.
 *
 * Plugins use this to extend the type system with new {@link Data}
 * subclasses without modifying nvisy-core.
 */
export interface DatatypeDescriptor {
	/** Unique identifier for this data type (e.g. "audio", "image"). */
	readonly id: string;
	/** Class reference for the custom data type. */
	readonly dataClass: ClassRef<Data>;
}

/** Factory for creating data type entries. */
export const Datatype = {
	/** Create a DatatypeDescriptor for registering a custom data type with a plugin. */
	define(id: string, dataClass: ClassRef<Data>): DatatypeDescriptor {
		return { id, dataClass };
	},
} as const;
