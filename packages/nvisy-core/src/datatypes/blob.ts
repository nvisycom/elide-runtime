/**
 * Binary blob data type for files from object storage.
 *
 * @module
 */

import { filetypeinfo } from "magic-bytes.js";
import { Data } from "./data.js";

/** Extension and MIME type pair describing a file type. */
export interface Filetype {
	/** File extension including the dot (e.g. `".pdf"`). */
	readonly extension?: string;
	/** MIME type (e.g. `"application/pdf"`). */
	readonly mime?: string;
}

/** Options for constructing a {@link Blob}. */
export interface BlobOptions {
	/** MIME type declared by the source (e.g. `"application/pdf"`). */
	readonly contentType?: string;
	/** Timestamp when the object was created in the source store. */
	readonly createdAt?: Date;
	/** Timestamp when the object was last modified in the source store. */
	readonly updatedAt?: Date;
}

/**
 * A file or binary blob retrieved from object storage (S3, GCS, Dropbox, etc.).
 *
 * Wraps raw bytes together with their storage path so downstream
 * processors can decide how to parse the content. File-type information
 * is available via two {@link Filetype} getters:
 *
 * - {@link provided} — declared type from the path extension and the
 *   cloud-provider / caller-supplied `contentType`.
 * - {@link identified} — detected type from the actual bytes via
 *   magic-bytes signatures (lazy, cached on first access).
 *
 * @example
 * ```ts
 * const blob = new Blob("uploads/report.pdf", pdfBytes, {
 *   contentType: "application/pdf",
 * });
 * blob.provided;   // { extension: ".pdf", mime: "application/pdf" }
 * blob.identified; // { extension: ".pdf", mime: "application/pdf" }
 * ```
 */
export class Blob extends Data {
	readonly #path: string;
	readonly #data: Buffer;
	readonly #filetype: Filetype;
	readonly #createdAt?: Date | undefined;
	readonly #updatedAt?: Date | undefined;

	// Lazy magic-bytes cache — `false` means "not yet computed"
	#identified: false | Filetype = false;

	constructor(path: string, data: Buffer, options?: BlobOptions) {
		super();
		this.#path = path;
		this.#data = data;
		this.#createdAt = options?.createdAt;
    this.#updatedAt = options?.updatedAt;
		
		const ext = Blob.#parseExtension(path);
		this.#filetype = {
			...(ext && { extension: ext }),
			...(options?.contentType && { mime: options.contentType }),
		};
	}

	/** Storage path or key (e.g. `"s3://bucket/file.pdf"`). */
	get path(): string {
		return this.#path;
	}

	/** Raw binary content. */
	get data(): Buffer {
		return this.#data;
	}

	/** Size of the raw data in bytes. */
	get size(): number {
		return this.#data.byteLength;
	}

	/** Timestamp when the object was created in the source store. */
	get createdAt(): Date | undefined {
		return this.#createdAt;
	}

	/** Timestamp when the object was last modified in the source store. */
	get updatedAt(): Date | undefined {
		return this.#updatedAt;
	}

	/** Declared file type derived from path extension and constructor contentType. */
	get provided(): Filetype {
		return this.#filetype;
	}

	/** File type detected from magic bytes. Fields are absent when bytes are not recognizable. */
	get identified(): Filetype {
		return this.#identify();
	}

	#identify(): Filetype {
		if (this.#identified === false) {
			const detected = filetypeinfo(this.#data);
			const first = detected[0];
			this.#identified = first
				? {
						...(first.extension && { extension: `.${first.extension}` }),
						...(first.mime && { mime: first.mime }),
					}
				: {};
		}
		return this.#identified;
	}

	static #parseExtension(path: string): string | undefined {
		const lastDot = path.lastIndexOf(".");
		if (lastDot === -1 || lastDot === path.length - 1) return undefined;
		return path.slice(lastDot).toLowerCase();
	}
}
