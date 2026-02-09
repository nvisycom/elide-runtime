import { getLogger } from "@logtape/logtape";
import type {
	AnyActionInstance,
	AnyLoaderInstance,
	AnyProviderFactory,
	AnyStreamSource,
	AnyStreamTarget,
	DatatypeDescriptor,
	PluginInstance,
} from "@nvisy/core";
import { type Blob, ValidationError } from "@nvisy/core";

const logger = getLogger(["nvisy", "registry"]);

/** Describes a single registered action for schema generation. */
export interface ActionDescriptor {
	readonly name: string;
	readonly configSchema: AnyActionInstance["schema"];
}

/** Describes a single registered provider for schema generation. */
export interface ProviderDescriptor {
	readonly name: string;
	readonly credentialSchema: AnyProviderFactory["credentialSchema"];
}

/**
 * Complete snapshot of everything currently registered,
 * suitable for generating an OpenAPI spec alongside the static
 * graph/node/edge schemas.
 */
export interface RegistrySchema {
	readonly actions: ReadonlyArray<ActionDescriptor>;
	readonly providers: ReadonlyArray<ProviderDescriptor>;
	readonly streams: number;
	readonly loaders: number;
	readonly datatypes: number;
}

/**
 * Unified registry that stores providers, actions, and loaders contributed by
 * {@link PluginInstance} objects.
 *
 * All entries are keyed as `"pluginId/name"`.
 */
export class Registry {
	readonly #actions = new Map<string, AnyActionInstance>();
	readonly #loaders = new Map<string, AnyLoaderInstance>();
	readonly #providers = new Map<string, AnyProviderFactory>();
	readonly #streams = new Map<string, AnyStreamSource | AnyStreamTarget>();
	readonly #datatypes = new Map<string, DatatypeDescriptor>();
	readonly #plugins = new Set<string>();

	/** Snapshot of all registered actions and providers with their schemas. */
	get schema(): RegistrySchema {
		const actions: ActionDescriptor[] = [];
		for (const [name, action] of this.#actions) {
			actions.push({ name, configSchema: action.schema });
		}

		const providers: ProviderDescriptor[] = [];
		for (const [name, factory] of this.#providers) {
			providers.push({ name, credentialSchema: factory.credentialSchema });
		}

		return {
			actions,
			providers,
			streams: this.#streams.size,
			loaders: this.#loaders.size,
			datatypes: this.#datatypes.size,
		};
	}

	/** Load all providers, actions, loaders, and streams declared by a plugin. */
	load(plugin: PluginInstance): void {
		if (this.#plugins.has(plugin.id)) {
			throw new ValidationError(`Plugin already loaded: ${plugin.id}`, {
				source: "registry",
				retryable: false,
				details: { pluginId: plugin.id },
			});
		}

		const maps = [
			["provider", this.#providers, plugin.providers],
			["action", this.#actions, plugin.actions],
			["loader", this.#loaders, plugin.loaders],
			["stream", this.#streams, plugin.streams],
			["datatype", this.#datatypes, plugin.datatypes],
		] as const;

		// Check for collisions across all maps
		const collisions: string[] = [];
		for (const [kind, map, entries] of maps) {
			for (const name of Object.keys(entries)) {
				const key = `${plugin.id}/${name}`;
				if (map.has(key)) {
					collisions.push(`${kind} "${key}"`);
				}
			}
		}

		if (collisions.length > 0) {
			logger.error(
				"Registry collision loading plugin {pluginId}: {collisions}",
				{ pluginId: plugin.id, collisions: collisions.join(", ") },
			);
			throw new ValidationError(
				`Registry collision: ${collisions.join(", ")}`,
				{
					source: "registry",
					retryable: false,
					details: { pluginId: plugin.id, collisions },
				},
			);
		}

		// Register all entries
		this.#plugins.add(plugin.id);
		const loaded: Record<string, string[]> = {};
		for (const [kind, map, entries] of maps) {
			const names: string[] = [];
			for (const [name, value] of Object.entries(entries)) {
				(map as Map<string, unknown>).set(`${plugin.id}/${name}`, value);
				names.push(name);
			}
			loaded[kind] = names;
		}

		const counts: Record<string, number> = {};
		for (const [kind, names] of Object.entries(loaded)) {
			if (names.length > 0) {
				counts[`${kind}s`] = names.length;
			}
		}
		logger.debug("Plugin loaded: {plugin}", { plugin: plugin.id, ...counts });
	}

	/** Look up an action by name. */
	getAction(name: string): AnyActionInstance {
		return this.#getOrThrow(this.#actions, name, "action");
	}

	/** Look up a provider factory by name. */
	getProvider(name: string): AnyProviderFactory {
		return this.#getOrThrow(this.#providers, name, "provider");
	}

	/** Look up a stream by name. */
	getStream(name: string): AnyStreamSource | AnyStreamTarget {
		return this.#getOrThrow(this.#streams, name, "stream");
	}

	/** Look up a loader by name. */
	getLoader(name: string): AnyLoaderInstance {
		return this.#getOrThrow(this.#loaders, name, "loader");
	}

	/** Look up a data type by name. */
	getDataType(name: string): DatatypeDescriptor {
		return this.#getOrThrow(this.#datatypes, name, "datatype");
	}

	/** Look up an action by name, returning undefined if not found. */
	findAction(name: string): AnyActionInstance | undefined {
		return this.#actions.get(name);
	}

	/** Look up a loader by name, returning undefined if not found. */
	findLoader(name: string): AnyLoaderInstance | undefined {
		return this.#loaders.get(name);
	}

	/** Look up a provider factory by name, returning undefined if not found. */
	findProvider(name: string): AnyProviderFactory | undefined {
		return this.#providers.get(name);
	}

	/** Look up a stream by name, returning undefined if not found. */
	findStream(name: string): (AnyStreamSource | AnyStreamTarget) | undefined {
		return this.#streams.get(name);
	}

	/** Look up a data type by name, returning undefined if not found. */
	findDataType(name: string): DatatypeDescriptor | undefined {
		return this.#datatypes.get(name);
	}

	/**
	 * Find a loader that matches the given blob by content type, magic bytes, or extension.
	 *
	 * Matching priority:
	 * 1. If blob has provided MIME (contentType), match by contentType first
	 * 2. Match by identified (magic bytes) extension
	 * 3. Fall back to provided extension from blob path
	 */
	findLoaderForBlob(blob: Blob): AnyLoaderInstance | undefined {
		const provided = blob.provided;
		if (provided.mime) {
			for (const loader of this.#loaders.values()) {
				if (loader.contentTypes.includes(provided.mime)) {
					return loader;
				}
			}
		}

		const identified = blob.identified;
		if (identified.extension) {
			for (const loader of this.#loaders.values()) {
				if (loader.extensions.includes(identified.extension)) {
					return loader;
				}
			}
		}

		if (provided.extension) {
			for (const loader of this.#loaders.values()) {
				if (loader.extensions.includes(provided.extension)) {
					return loader;
				}
			}
		}

		return undefined;
	}

	#getOrThrow<T>(map: Map<string, T>, name: string, kind: string): T {
		const entry = map.get(name);
		if (!entry) {
			logger.warn(`${kind} not found: ${name}`, { [kind]: name });
			throw ValidationError.notFound(name, kind, "registry");
		}
		return entry;
	}
}
