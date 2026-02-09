/**
 * Plugin system for bundling providers, streams, actions, loaders,
 * and custom datatypes under a single namespace.
 *
 * Plugins are the unit of registration with the engine. Use
 * {@link Plugin.define} to create a new plugin, then chain
 * `.withProviders()`, `.withActions()`, etc. to populate it.
 *
 * @module
 */

import type { ActionInstance } from "./action.js";
import type { DatatypeDescriptor } from "./datatypes/index.js";
import type { LoaderInstance } from "./loader.js";
import type { ProviderFactory } from "./provider.js";
import type { StreamSource, StreamTarget } from "./stream.js";

// biome-ignore lint/suspicious/noExplicitAny: existential type alias
export type AnyProviderFactory = ProviderFactory<any, any>;

// biome-ignore lint/suspicious/noExplicitAny: existential type alias
export type AnyActionInstance = ActionInstance<any, any, any, any>;

// biome-ignore lint/suspicious/noExplicitAny: existential type alias
export type AnyLoaderInstance = LoaderInstance<any>;

// biome-ignore lint/suspicious/noExplicitAny: existential type alias
export type AnyStreamSource = StreamSource<any, any, any, any>;

// biome-ignore lint/suspicious/noExplicitAny: existential type alias
export type AnyStreamTarget = StreamTarget<any, any, any>;

/**
 * A plugin bundles providers, streams, actions, and loaders under a namespace.
 *
 * Plugins are the unit of registration with the engine. All entries
 * are namespaced as `"pluginId/name"` to avoid collisions.
 */
export interface PluginInstance {
	/** Unique identifier for the plugin (e.g. "sql", "openai"). */
	readonly id: string;
	/** Provider factories keyed by their ID. */
	readonly providers: Readonly<Record<string, AnyProviderFactory>>;
	/** Stream sources and targets keyed by their ID. */
	readonly streams: Readonly<Record<string, AnyStreamSource | AnyStreamTarget>>;
	/** Actions keyed by their ID. */
	readonly actions: Readonly<Record<string, AnyActionInstance>>;
	/** Loaders keyed by their ID. */
	readonly loaders: Readonly<Record<string, AnyLoaderInstance>>;
	/** Custom data types keyed by their ID. */
	readonly datatypes: Readonly<Record<string, DatatypeDescriptor>>;
}

class PluginBuilder implements PluginInstance {
	readonly id: string;
	readonly providers: Readonly<Record<string, AnyProviderFactory>> = {};
	readonly streams: Readonly<
		Record<string, AnyStreamSource | AnyStreamTarget>
	> = {};
	readonly actions: Readonly<Record<string, AnyActionInstance>> = {};
	readonly loaders: Readonly<Record<string, AnyLoaderInstance>> = {};
	readonly datatypes: Readonly<Record<string, DatatypeDescriptor>> = {};

	constructor(id: string) {
		this.id = id;
	}

	/** Add providers to this plugin. */
	withProviders(...providers: AnyProviderFactory[]): this {
		const record = { ...this.providers };
		for (const p of providers) record[p.id] = p;
		(this as { providers: typeof record }).providers = record;
		return this;
	}

	/** Add streams to this plugin. */
	withStreams(...streams: (AnyStreamSource | AnyStreamTarget)[]): this {
		const record = { ...this.streams };
		for (const s of streams) record[s.id] = s;
		(this as { streams: typeof record }).streams = record;
		return this;
	}

	/** Add actions to this plugin. */
	withActions(...actions: AnyActionInstance[]): this {
		const record = { ...this.actions };
		for (const a of actions) record[a.id] = a;
		(this as { actions: typeof record }).actions = record;
		return this;
	}

	/** Add loaders to this plugin. */
	withLoaders(...loaders: AnyLoaderInstance[]): this {
		const record = { ...this.loaders };
		for (const l of loaders) record[l.id] = l;
		(this as { loaders: typeof record }).loaders = record;
		return this;
	}

	/** Add custom data types to this plugin. */
	withDatatypes(...datatypes: DatatypeDescriptor[]): this {
		const record = { ...this.datatypes };
		for (const d of datatypes) record[d.id] = d;
		(this as { datatypes: typeof record }).datatypes = record;
		return this;
	}
}

/** Factory for creating plugin definitions. */
export const Plugin = {
	/** Create a new plugin with the given ID. */
	define(id: string): PluginBuilder {
		return new PluginBuilder(id);
	},
} as const;
