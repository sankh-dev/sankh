export type Segment = string | number;
export type AnnotationKind = 'expect' | 'capture';

export const EXPECT_OPS = ['==', '!=', 'exists', 'contains', 'matches', '>', '>=', '<', '<=', 'length >'] as const;
export type ExpectOp = (typeof EXPECT_OPS)[number];

export interface JsonPick {
	kind: AnnotationKind;
	segments: Segment[];
	value: unknown;
	/** The clicked button, to place the popover under. */
	anchor: DOMRect;
}

const IDENT = /^[A-Za-z_][A-Za-z0-9_]*$/;
/** How the server shows secret values in responses. */
export const MASK = '***';
export const CAPTURE_NAME = /^[A-Z_][A-Z0-9_]*$/;

/** A jq path such as `.data.items[0]["x-y"]`; `.` for the root. */
export function jqPath(segments: Segment[]): string {
	let out = '';
	for (const s of segments) {
		if (typeof s === 'number') out += `[${s}]`;
		else if (IDENT.test(s)) out += `.${s}`;
		else out += `[${JSON.stringify(s)}]`;
	}
	if (out === '') return '.';
	return out.startsWith('.') ? out : `.${out}`;
}

/** The value as a one-line JSON literal, as `@expect json` takes it. */
export function jqLiteral(value: unknown): string {
	return JSON.stringify(value) ?? 'null';
}

/** UPPER_SNAKE name from the last object key, e.g. `.data.accessToken` -> `ACCESS_TOKEN`. */
export function captureName(segments: Segment[]): string {
	const key = [...segments].reverse().find((s): s is string => typeof s === 'string') ?? 'value';
	const name = key
		.replace(/([a-z0-9])([A-Z])/g, '$1_$2')
		.replace(/[^A-Za-z0-9]+/g, '_')
		.replace(/^_+|_+$/g, '')
		.toUpperCase();
	if (!name) return 'VALUE';
	return /^[0-9]/.test(name) ? `_${name}` : name;
}

/** Arguments of `@expect`, e.g. `json .data.id == 7`. */
export function expectLine(path: string, op: ExpectOp, value: string): string {
	if (op === 'exists') return `json ${path} exists`;
	if (op === 'length >') return `json ${path} | length > ${value}`;
	return `json ${path} ${op} ${value}`;
}

/** Arguments of `@capture`, e.g. `TOKEN=.data.token`. */
export function captureLine(name: string, path: string): string {
	return `${name}=${path}`;
}
