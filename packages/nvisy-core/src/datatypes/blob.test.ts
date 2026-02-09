import { describe, expect, it } from "vitest";
import { Blob } from "./blob.js";

describe("Blob", () => {
	it("stores path and data", () => {
		const data = Buffer.from("hello world");
		const blob = new Blob("uploads/file.txt", data);
		expect(blob.path).toBe("uploads/file.txt");
		expect(blob.data).toBe(data);
		expect(blob.data.toString()).toBe("hello world");
	});

	it("provided.mime is undefined when no contentType given", () => {
		const blob = new Blob("file.bin", Buffer.from([0x00, 0x01]));
		expect(blob.provided.mime).toBeUndefined();
	});

	it("provided.mime reflects constructor contentType", () => {
		const blob = new Blob("report.pdf", Buffer.from("pdf content"), {
			contentType: "application/pdf",
		});
		expect(blob.provided.mime).toBe("application/pdf");
	});

	it("size returns byte length of data", () => {
		const blob = new Blob("file.txt", Buffer.from("abc"));
		expect(blob.size).toBe(3);
	});

	it("size handles empty buffer", () => {
		const blob = new Blob("empty.bin", Buffer.alloc(0));
		expect(blob.size).toBe(0);
	});

	it("size handles binary data correctly", () => {
		const binaryData = Buffer.from([0x00, 0xff, 0x10, 0x20, 0x30]);
		const blob = new Blob("binary.bin", binaryData);
		expect(blob.size).toBe(5);
	});

	it("extends Data and has id, parentId, metadata", () => {
		const blob = new Blob("file.txt", Buffer.from("content"));
		expect(blob.id).toMatch(
			/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/,
		);
		expect(blob.parentId).toBeNull();
		expect(blob.metadata).toBeNull();
	});

	it("supports deriveFrom for lineage", () => {
		const parent = new Blob("parent.txt", Buffer.from("parent"));
		const child = new Blob("child.txt", Buffer.from("child")).deriveFrom(
			parent,
		);
		expect(child.parentId).toBe(parent.id);
		expect(child.isDerived).toBe(true);
	});

	it("supports withMetadata", () => {
		const blob = new Blob("file.txt", Buffer.from("content")).withMetadata({
			source: "s3",
			bucket: "my-bucket",
		});
		expect(blob.metadata).toEqual({ source: "s3", bucket: "my-bucket" });
	});

	describe("createdAt / updatedAt", () => {
		it("defaults to undefined when not provided", () => {
			const blob = new Blob("file.txt", Buffer.from(""));
			expect(blob.createdAt).toBeUndefined();
			expect(blob.updatedAt).toBeUndefined();
		});

		it("stores and returns the dates when provided", () => {
			const created = new Date("2025-01-01T00:00:00Z");
			const updated = new Date("2025-06-15T12:00:00Z");
			const blob = new Blob("file.txt", Buffer.from(""), {
				createdAt: created,
				updatedAt: updated,
			});
			expect(blob.createdAt).toBe(created);
			expect(blob.updatedAt).toBe(updated);
		});
	});

	describe("provided", () => {
		it("extracts extension from path", () => {
			const blob = new Blob("report.pdf", Buffer.from(""));
			expect(blob.provided.extension).toBe(".pdf");
		});

		it("includes mime from contentType", () => {
			const blob = new Blob("report.pdf", Buffer.from(""), {
				contentType: "application/pdf",
			});
			expect(blob.provided.mime).toBe("application/pdf");
		});

		it("omits extension for extensionless path", () => {
			const blob = new Blob("Makefile", Buffer.from(""));
			expect(blob.provided.extension).toBeUndefined();
		});

		it("lowercases the extension", () => {
			const blob = new Blob("photo.JPG", Buffer.from(""));
			expect(blob.provided.extension).toBe(".jpg");
		});

		it("handles paths with multiple dots", () => {
			const blob = new Blob("archive.tar.gz", Buffer.from(""));
			expect(blob.provided.extension).toBe(".gz");
		});
	});

	describe("identified", () => {
		it("detects PDF from magic bytes", () => {
			const pdfHeader = Buffer.from("%PDF-1.4 ...");
			const blob = new Blob("mystery.bin", pdfHeader);
			expect(blob.identified.extension).toBe(".pdf");
			expect(blob.identified.mime).toBe("application/pdf");
		});

		it("returns empty filetype for unrecognizable bytes (e.g. CSV)", () => {
			const blob = new Blob("data.csv", Buffer.from("a,b\n1,2"));
			expect(blob.identified.extension).toBeUndefined();
			expect(blob.identified.mime).toBeUndefined();
		});
	});

	it("handles various path formats", () => {
		const s3Blob = new Blob("s3://bucket/key/file.pdf", Buffer.from(""));
		expect(s3Blob.path).toBe("s3://bucket/key/file.pdf");

		const gcsBlob = new Blob("gs://bucket/object", Buffer.from(""));
		expect(gcsBlob.path).toBe("gs://bucket/object");

		const localBlob = new Blob("/var/data/file.txt", Buffer.from(""));
		expect(localBlob.path).toBe("/var/data/file.txt");
	});
});
