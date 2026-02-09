/**
 * Stream sources and targets for reading from and writing to external systems.
 *
 * Sources are pipeline entry points that emit {@link Resumable} items
 * for crash recovery. Targets are pipeline exit points that persist
 * processed data. Use {@link Stream.createSource} and
 * {@link Stream.createTarget} to define new endpoints.
 *
 * @module
 */

import type { z } from "zod";
import type { Data } from "./datatypes/index.js";
import type { ClassRef } from "./types.js";

/**
 * A data item paired with resumption context.
 *
 * Stream sources emit resumables so that the engine can persist
 * context after each item, enabling crash recovery.
 *
 * @template TData - The data type being streamed.
 * @template TCtx - Context type for resumption (e.g. cursor, offset).
 */
export interface Resumable<TData extends Data = Data, TCtx = void> {
	/** The data item being streamed. */
	readonly data: TData;
	/** Context for resuming from this point. */
	readonly context: TCtx;
}

/**
 * Function that reads data from an external system.
 *
 * @template TClient - Provider client type for connecting to the source.
 * @template TData - Data type produced by the reader.
 * @template TCtx - Context type for resumption (e.g. cursor, offset).
 * @template TParam - Configuration parameters for the reader.
 */
export type ReaderFn<TClient, TData extends Data, TCtx, TParam> = (
	client: TClient,
	ctx: TCtx,
	params: TParam,
) => AsyncIterable<Resumable<TData, TCtx>>;

/**
 * Function that returns a writer for persisting data items.
 *
 * @template TClient - Provider client type for connecting to the target.
 * @template TData - Data type consumed by the writer.
 * @template TParam - Configuration parameters for the writer.
 */
export type WriterFn<TClient, TData extends Data, TParam> = (
	client: TClient,
	params: TParam,
) => (item: TData) => Promise<void>;

/**
 * Configuration for creating a stream source.
 *
 * @template TClient - Provider client type for connecting to the source.
 * @template TData - Data type produced by the source.
 * @template TCtx - Context type for resumption.
 * @template TParam - Configuration parameters for the source.
 */
export interface SourceConfig<TClient, TData extends Data, TCtx, TParam> {
	/** Class reference for the data type produced. */
	readonly type: ClassRef<TData>;
	/** Zod schema for validating and parsing resumption context. */
	readonly context: z.ZodType<TCtx>;
	/** Zod schema for validating stream parameters. */
	readonly params: z.ZodType<TParam>;
	/** The reader function that produces data items. */
	readonly reader: ReaderFn<TClient, TData, TCtx, TParam>;
}

/**
 * Configuration for creating a stream target.
 *
 * @template TClient - Provider client type for connecting to the target.
 * @template TData - Data type consumed by the target.
 * @template TParam - Configuration parameters for the target.
 */
export interface TargetConfig<TClient, TData extends Data, TParam> {
	/** Class reference for the data type consumed. */
	readonly type: ClassRef<TData>;
	/** Zod schema for validating stream parameters. */
	readonly params: z.ZodType<TParam>;
	/** The writer function that persists data items. */
	readonly writer: WriterFn<TClient, TData, TParam>;
}

/**
 * A stream source that reads data from an external system.
 *
 * Sources are the entry points of a pipeline, producing data items
 * that flow through actions to targets.
 *
 * @template TClient - Provider client type for connecting to the source.
 * @template TData - Data type produced by the source.
 * @template TCtx - Context type for resumption.
 * @template TParam - Configuration parameters for the source.
 */
export interface StreamSource<
	TClient,
	TData extends Data,
	TCtx,
	TParam = void,
> {
	/** Discriminator for runtime type checking. */
	readonly kind: "source";
	/** Unique identifier for this stream source. */
	readonly id: string;
	/** Class reference for the required provider client. */
	readonly clientClass: ClassRef<TClient>;
	/** Class reference for the data type produced. */
	readonly dataClass: ClassRef<TData>;
	/** Zod schema for validating and parsing resumption context. */
	readonly contextSchema: z.ZodType<TCtx>;
	/** Zod schema for validating stream parameters. */
	readonly paramSchema: z.ZodType<TParam>;
	/** Read data from the source, yielding resumable items. */
	read(
		client: TClient,
		ctx: TCtx,
		params: TParam,
	): AsyncIterable<Resumable<TData, TCtx>>;
}

/**
 * A stream target that writes data to an external system.
 *
 * Targets are the exit points of a pipeline, persisting data items
 * that have flowed from sources through actions.
 *
 * @template TClient - Provider client type for connecting to the target.
 * @template TData - Data type consumed by the target.
 * @template TParam - Configuration parameters for the target.
 */
export interface StreamTarget<TClient, TData extends Data, TParam = void> {
	/** Discriminator for runtime type checking. */
	readonly kind: "target";
	/** Unique identifier for this stream target. */
	readonly id: string;
	/** Class reference for the required provider client. */
	readonly clientClass: ClassRef<TClient>;
	/** Class reference for the data type consumed. */
	readonly dataClass: ClassRef<TData>;
	/** Zod schema for validating stream parameters. */
	readonly paramSchema: z.ZodType<TParam>;
	/** Create a writer function for persisting items. */
	write(client: TClient, params: TParam): (item: TData) => Promise<void>;
}

class StreamSourceImpl<TClient, TData extends Data, TCtx, TParam>
	implements StreamSource<TClient, TData, TCtx, TParam>
{
	readonly kind = "source" as const;
	readonly id: string;
	readonly clientClass: ClassRef<TClient>;
	readonly dataClass: ClassRef<TData>;
	readonly contextSchema: z.ZodType<TCtx>;
	readonly paramSchema: z.ZodType<TParam>;
	readonly #read: ReaderFn<TClient, TData, TCtx, TParam>;

	constructor(config: {
		id: string;
		clientClass: ClassRef<TClient>;
		dataClass: ClassRef<TData>;
		contextSchema: z.ZodType<TCtx>;
		paramSchema: z.ZodType<TParam>;
		read: ReaderFn<TClient, TData, TCtx, TParam>;
	}) {
		this.id = config.id;
		this.clientClass = config.clientClass;
		this.dataClass = config.dataClass;
		this.contextSchema = config.contextSchema;
		this.paramSchema = config.paramSchema;
		this.#read = config.read;
	}

	read(
		client: TClient,
		ctx: TCtx,
		params: TParam,
	): AsyncIterable<Resumable<TData, TCtx>> {
		return this.#read(client, ctx, params);
	}
}

class StreamTargetImpl<TClient, TData extends Data, TParam>
	implements StreamTarget<TClient, TData, TParam>
{
	readonly kind = "target" as const;
	readonly id: string;
	readonly clientClass: ClassRef<TClient>;
	readonly dataClass: ClassRef<TData>;
	readonly paramSchema: z.ZodType<TParam>;
	readonly #writer: WriterFn<TClient, TData, TParam>;

	constructor(config: {
		id: string;
		clientClass: ClassRef<TClient>;
		dataClass: ClassRef<TData>;
		paramSchema: z.ZodType<TParam>;
		writer: WriterFn<TClient, TData, TParam>;
	}) {
		this.id = config.id;
		this.clientClass = config.clientClass;
		this.dataClass = config.dataClass;
		this.paramSchema = config.paramSchema;
		this.#writer = config.writer;
	}

	write(client: TClient, params: TParam): (item: TData) => Promise<void> {
		return this.#writer(client, params);
	}
}

/** Factory for creating stream sources and targets. */
export const Stream = {
	/**
	 * Create a stream source for reading data from an external system.
	 *
	 * @param id - Unique identifier for the stream source.
	 * @param clientClass - Class reference for the required provider client.
	 * @param config - Source configuration including types and reader function.
	 */
	createSource<TClient, TData extends Data, TCtx, TParam>(
		id: string,
		clientClass: ClassRef<TClient>,
		config: SourceConfig<TClient, TData, TCtx, TParam>,
	): StreamSource<TClient, TData, TCtx, TParam> {
		const {
			type: dataClass,
			context: contextSchema,
			params: paramSchema,
		} = config;
		return new StreamSourceImpl({
			id,
			clientClass,
			dataClass,
			contextSchema,
			paramSchema,
			read: config.reader,
		});
	},

	/**
	 * Create a stream target for writing data to an external system.
	 *
	 * @param id - Unique identifier for the stream target.
	 * @param clientClass - Class reference for the required provider client.
	 * @param config - Target configuration including types and writer function.
	 */
	createTarget<TClient, TData extends Data, TParam>(
		id: string,
		clientClass: ClassRef<TClient>,
		config: TargetConfig<TClient, TData, TParam>,
	): StreamTarget<TClient, TData, TParam> {
		const { type: dataClass, params: paramSchema } = config;
		return new StreamTargetImpl({
			id,
			clientClass,
			dataClass,
			paramSchema,
			writer: config.writer,
		});
	},
} as const;
