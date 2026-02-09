export interface RegexSplitOptions {
	/** Pattern to split on. Compiled to a RegExp with the `gm` flags. */
	readonly pattern: string;
	/** If true, keep the matched separator at the start of each subsequent segment. Default: false. */
	readonly keepSeparator?: boolean;
	/** Discard segments that are empty or whitespace-only after splitting. Default: true. */
	readonly trimEmpty?: boolean;
}

/** Split `text` on a regex pattern. */
export function splitByRegex(
	text: string,
	options: RegexSplitOptions,
): string[] {
	const { pattern, keepSeparator = false, trimEmpty = true } = options;

	const re = new RegExp(pattern, "gm");

	// Collect all match boundaries
	const boundaries: { start: number; end: number }[] = [];
	for (let match = re.exec(text); match !== null; match = re.exec(text)) {
		if (match[0].length === 0) {
			re.lastIndex++;
			continue;
		}
		boundaries.push({ start: match.index, end: match.index + match[0].length });
	}

	if (boundaries.length === 0) {
		const result = trimEmpty && text.trim().length === 0 ? [] : [text];
		return result;
	}

	const segments: string[] = [];

	if (keepSeparator) {
		// First segment: everything before the first match
		segments.push(text.slice(0, boundaries[0]!.start));
		// Subsequent segments start at each match start, end at next match start (or end of text)
		for (let i = 0; i < boundaries.length; i++) {
			const segStart = boundaries[i]!.start;
			const segEnd =
				i + 1 < boundaries.length ? boundaries[i + 1]!.start : text.length;
			segments.push(text.slice(segStart, segEnd));
		}
	} else {
		let cursor = 0;
		for (const b of boundaries) {
			segments.push(text.slice(cursor, b.start));
			cursor = b.end;
		}
		segments.push(text.slice(cursor));
	}

	if (trimEmpty) {
		return segments.filter((s) => s.trim().length > 0);
	}
	return segments;
}
