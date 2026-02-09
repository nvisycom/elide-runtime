import { describe, expect, it } from "vitest";
import { splitByRegex } from "./regex.js";

describe("splitByRegex", () => {
	it("splits on a simple pattern", () => {
		const result = splitByRegex("a1b2c", { pattern: "\\d" });
		expect(result).toEqual(["a", "b", "c"]);
	});

	it("splits on a multi-character pattern", () => {
		const result = splitByRegex("hello---world---end", { pattern: "-+" });
		expect(result).toEqual(["hello", "world", "end"]);
	});

	it("splits on newline patterns", () => {
		const result = splitByRegex("line1\n\nline2\n\nline3", {
			pattern: "\\n{2,}",
		});
		expect(result).toEqual(["line1", "line2", "line3"]);
	});

	it("returns [text] when pattern does not match", () => {
		const result = splitByRegex("no match here", { pattern: "\\d+" });
		expect(result).toEqual(["no match here"]);
	});

	it("returns empty array for empty input (trimEmpty=true)", () => {
		const result = splitByRegex("", { pattern: "\\d" });
		expect(result).toEqual([]);
	});

	it("returns [''] for empty input when trimEmpty=false", () => {
		const result = splitByRegex("", { pattern: "\\d", trimEmpty: false });
		expect(result).toEqual([""]);
	});

	describe("keepSeparator", () => {
		it("prepends matched separator to subsequent segments", () => {
			const result = splitByRegex("intro\n## A\ncontent A\n## B\ncontent B", {
				pattern: "^## .+$",
				keepSeparator: true,
			});
			expect(result).toEqual([
				"intro\n",
				"## A\ncontent A\n",
				"## B\ncontent B",
			]);
		});

		it("keeps separator with simple pattern", () => {
			const result = splitByRegex("a1b2c", {
				pattern: "\\d",
				keepSeparator: true,
			});
			expect(result).toEqual(["a", "1b", "2c"]);
		});
	});

	describe("trimEmpty", () => {
		it("filters whitespace-only segments by default", () => {
			const result = splitByRegex("a,,b", { pattern: "," });
			expect(result).toEqual(["a", "b"]);
		});

		it("keeps whitespace-only segments when trimEmpty=false", () => {
			const result = splitByRegex("a,,b", {
				pattern: ",",
				trimEmpty: false,
			});
			expect(result).toEqual(["a", "", "b"]);
		});
	});

	it("handles consecutive separators", () => {
		const result = splitByRegex("a--b--c", { pattern: "-" });
		expect(result).toEqual(["a", "b", "c"]);
	});

	it("handles pattern at start and end", () => {
		const result = splitByRegex("1a1b1", { pattern: "\\d" });
		expect(result).toEqual(["a", "b"]);
	});

	it("uses multiline flag so ^ matches line starts", () => {
		const result = splitByRegex("line1\nline2\nline3", {
			pattern: "^line2$",
		});
		expect(result).toEqual(["line1\n", "\nline3"]);
	});
});
