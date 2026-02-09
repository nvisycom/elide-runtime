/**
 * @module @nvisy/core
 *
 * Public API surface for the nvisy core library.
 */

export type { ActionInstance } from "./action.js";
export { Action } from "./action.js";
export type {
	BlobOptions,
	ChunkOptions,
	DatatypeDescriptor,
	DocumentOptions,
	Filetype,
} from "./datatypes/index.js";
export {
	Blob,
	Chunk,
	Data,
	Datatype,
	Document,
	Embedding,
} from "./datatypes/index.js";
export type {
	CompositeElementOptions,
	ElementOptions,
	ElementProvenance,
	EmailElementOptions,
	EmphasizedText,
	FormElementOptions,
	FormKeyValuePair,
	ImageElementOptions,
	Link,
	TableCellData,
	TableElementOptions,
} from "./documents/elements.js";
export {
	CompositeElement,
	Element,
	EmailElement,
	FormElement,
	ImageElement,
	TableElement,
} from "./documents/elements.js";
export type {
	ElementCategory,
	ElementCoordinates,
	Orientation,
	Point,
} from "./documents/index.js";
export {
	CodeType,
	CoordinateSystem,
	categoryOf,
	ElementType,
	EmailType,
	FormType,
	LayoutType,
	MathType,
	MediaType,
	Orientations,
	ontology,
	TableType,
	TextType,
} from "./documents/index.js";
export type { ErrorContext } from "./errors/index.js";
export {
	CancellationError,
	ConnectionError,
	RuntimeError,
	TimeoutError,
	ValidationError,
} from "./errors/index.js";
export type { LoaderConfig, LoaderInstance, LoadFn } from "./loader.js";
export { Loader } from "./loader.js";
export type {
	AnyActionInstance,
	AnyLoaderInstance,
	AnyProviderFactory,
	AnyStreamSource,
	AnyStreamTarget,
	PluginInstance,
} from "./plugin.js";
export { Plugin } from "./plugin.js";
export type {
	ConnectedInstance,
	ProviderFactory,
	ProviderInstance,
} from "./provider.js";
export { Provider } from "./provider.js";
export type {
	Resumable,
	StreamSource,
	StreamTarget,
	WriterFn,
} from "./stream.js";
export { Stream } from "./stream.js";
export type { ClassRef, JsonValue, Metadata } from "./types.js";
