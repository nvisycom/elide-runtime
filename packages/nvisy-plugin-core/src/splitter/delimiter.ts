export interface DelimiterSplitOptions {
	/** String to split on (e.g. `"\n"`, `"---"`). */
	readonly delimiter: string;
	/** If true, keep the delimiter at the start of each subsequent segment. Default: false. */
	readonly keepDelimiter?: boolean;
	/** Discard segments that are empty or whitespace-only after splitting. Default: true. */
	readonly trimEmpty?: boolean;
}

/** Split `text` on a literal delimiter string. */
export function splitByDelimiter(
	text: string,
	options: DelimiterSplitOptions,
): string[] {
	const { delimiter, keepDelimiter = false, trimEmpty = true } = options;

	const raw = text.split(delimiter);

	let segments: string[];
	if (keepDelimiter) {
		segments = raw.map((seg, i) => (i === 0 ? seg : `${delimiter}${seg}`));
	} else {
		segments = raw;
	}

	if (trimEmpty) {
		segments = segments.filter((s) => s.trim().length > 0);
	}

	return segments;
}
