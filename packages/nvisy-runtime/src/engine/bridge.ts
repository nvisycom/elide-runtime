/**
 * Loader bridge for automatic Blob → Document conversion.
 *
 * When a source node produces {@link Blob}s but downstream action or
 * target nodes expect {@link Document}s, the bridge transparently
 * selects a matching loader from the registry (by file extension and
 * magic-byte content type), converts each blob, and yields the
 * resulting documents. Converted documents are cached by blob ID in a
 * per-run {@link LoaderCache} so the same blob is never loaded twice
 * even when consumed by multiple downstream branches.
 *
 * @module
 */

import { getLogger } from "@logtape/logtape";
import { Blob, type Data, type Document, RuntimeError } from "@nvisy/core";
import type { Registry } from "../registry.js";

const logger = getLogger(["nvisy", "bridge"]);

/** Cache for converted documents, shared across an execution run. */
export type LoaderCache = Map<string, Document[]>;

/** Creates a new empty loader cache for an execution run. */
export function createLoaderCache(): LoaderCache {
	return new Map();
}

/** Options for the loader bridge. */
export interface BridgeOptions {
	/** When true, skip blobs with no matching loader instead of throwing. */
	readonly ignoreUnsupported?: boolean;
}

/**
 * Wrap an async iterable to automatically convert Blobs to Documents.
 *
 * Non-Blob items pass through unchanged. For each Blob the registry
 * is queried for a loader that matches the file's extension / content
 * type. If no loader is found, behaviour depends on
 * {@link BridgeOptions.ignoreUnsupported}: when true the blob is
 * silently dropped; otherwise a {@link RuntimeError} is thrown.
 *
 * @param stream - Upstream data items (may contain a mix of Blobs and other types).
 * @param registry - Used to look up loaders by extension / magic bytes.
 * @param cache - Per-run cache; blobs already converted are yielded from cache.
 * @param options - Optional bridge configuration.
 */
export async function* applyLoaderBridge(
	stream: AsyncIterable<Data>,
	registry: Registry,
	cache: LoaderCache,
	options?: BridgeOptions,
): AsyncIterable<Data> {
	for await (const item of stream) {
		if (!(item instanceof Blob)) {
			yield item;
			continue;
		}

		const cached = cache.get(item.id);
		if (cached) {
			logger.debug("Using cached documents for blob {id}", { id: item.id });
			for (const doc of cached) {
				yield doc;
			}
			continue;
		}

		const loader = registry.findLoaderForBlob(item);

		if (!loader) {
			if (options?.ignoreUnsupported) {
				logger.warn("No loader found for blob {path}, skipping", {
					path: item.path,
				});
				continue;
			}
			throw new RuntimeError(
				`No loader found for blob: ${item.path} (mime: ${item.provided.mime ?? "unknown"})`,
				{ source: "bridge", retryable: false },
			);
		}

		logger.debug("Converting blob {path} using loader {loader}", {
			path: item.path,
			loader: loader.id,
		});

		const docs: Document[] = [];
		const params = loader.schema.parse({});
		for await (const doc of loader.load(item, params)) {
			docs.push(doc);
			yield doc;
		}
		cache.set(item.id, docs);
	}
}
