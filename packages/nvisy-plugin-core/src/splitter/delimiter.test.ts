import { describe, expect, it } from "vitest";
import { splitByDelimiter } from "./delimiter.js";

describe("splitByDelimiter", () => {
	it("splits on a simple delimiter", () => {
		const result = splitByDelimiter("a,b,c", { delimiter: "," });
		expect(result).toEqual(["a", "b", "c"]);
	});

	it("splits on a multi-character delimiter", () => {
		const result = splitByDelimiter("a---b---c", { delimiter: "---" });
		expect(result).toEqual(["a", "b", "c"]);
	});

	it("splits on newline delimiter", () => {
		const result = splitByDelimiter("line1\nline2\nline3", {
			delimiter: "\n",
		});
		expect(result).toEqual(["line1", "line2", "line3"]);
	});

	it("returns [text] when delimiter is not found", () => {
		const result = splitByDelimiter("no match here", { delimiter: "," });
		expect(result).toEqual(["no match here"]);
	});

	it("returns empty array for empty input (trimEmpty=true)", () => {
		const result = splitByDelimiter("", { delimiter: "," });
		expect(result).toEqual([]);
	});

	it("returns [''] for empty input when trimEmpty=false", () => {
		const result = splitByDelimiter("", {
			delimiter: ",",
			trimEmpty: false,
		});
		expect(result).toEqual([""]);
	});

	describe("keepDelimiter", () => {
		it("prepends delimiter to subsequent segments", () => {
			const result = splitByDelimiter("a,b,c", {
				delimiter: ",",
				keepDelimiter: true,
			});
			expect(result).toEqual(["a", ",b", ",c"]);
		});

		it("prepends multi-character delimiter", () => {
			const result = splitByDelimiter("a---b---c", {
				delimiter: "---",
				keepDelimiter: true,
			});
			expect(result).toEqual(["a", "---b", "---c"]);
		});
	});

	describe("trimEmpty", () => {
		it("filters whitespace-only segments by default", () => {
			const result = splitByDelimiter("a,, ,b", { delimiter: "," });
			expect(result).toEqual(["a", "b"]);
		});

		it("keeps whitespace-only segments when trimEmpty=false", () => {
			const result = splitByDelimiter("a,, ,b", {
				delimiter: ",",
				trimEmpty: false,
			});
			expect(result).toEqual(["a", "", " ", "b"]);
		});
	});

	it("handles consecutive delimiters", () => {
		const result = splitByDelimiter("a,,b", { delimiter: "," });
		expect(result).toEqual(["a", "b"]);
	});

	it("handles delimiter at start and end", () => {
		const result = splitByDelimiter(",a,b,", { delimiter: "," });
		expect(result).toEqual(["a", "b"]);
	});

	it("handles delimiter at start and end with trimEmpty=false", () => {
		const result = splitByDelimiter(",a,b,", {
			delimiter: ",",
			trimEmpty: false,
		});
		expect(result).toEqual(["", "a", "b", ""]);
	});
});
