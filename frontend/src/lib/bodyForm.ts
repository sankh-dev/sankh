import type { CurlCommand } from './types';

export type BodyMode = 'none' | 'raw' | 'form' | 'urlencoded' | 'file';

const FORM_FLAGS = ['-F', '--form', '--form-string'];
const URL_FLAG = '--data-urlencode';
/** Flags that take a body value; rendered one per line after the headers (`format::split_flags`). */
const VALUE_FLAGS = [...FORM_FLAGS, URL_FLAG];
/** Data flags left in `flags` when a command has several bodies. */
const DATA_FLAGS = ['-d', '--data', '--data-ascii', '--data-raw', '--data-binary', '--json'];

export interface BodyFlag {
	flag: string;
	arg: string;
}

/** `-Fname=v` keeps its value in the same word. */
function gluedForm(word: string) {
	return word.length > 2 && word.startsWith('-F') ? word.slice(2) : null;
}

/** Splits `flags` into those for the `curl` line and the `-F` / `--data-urlencode` pairs. */
export function splitFlags(flags: string[]): { line: string[]; body: BodyFlag[] } {
	const line: string[] = [];
	const body: BodyFlag[] = [];
	for (let i = 0; i < flags.length; i++) {
		const f = flags[i];
		const glued = gluedForm(f);
		if (glued !== null) body.push({ flag: '-F', arg: glued });
		else if (VALUE_FLAGS.includes(f)) body.push({ flag: f, arg: flags[++i] ?? '' });
		else line.push(f);
	}
	return { line, body };
}

export function joinFlags(line: string[], body: BodyFlag[]): string[] {
	return [...line, ...body.flatMap((b) => [b.flag, b.arg])];
}

/** The command sets its body with several data flags, kept verbatim in Settings. */
export function hasDataFlags(flags: string[]) {
	return flags.some((f) => DATA_FLAGS.includes(f));
}

export function isFileBody(curl: CurlCommand) {
	return curl.body_flag === '--data-binary' && (curl.body ?? '').startsWith('@');
}

export function bodyMode(curl: CurlCommand): BodyMode {
	const { body } = splitFlags(curl.flags);
	if (body.some((b) => FORM_FLAGS.includes(b.flag))) return 'form';
	if (body.some((b) => b.flag === URL_FLAG)) return 'urlencoded';
	if (isFileBody(curl)) return 'file';
	return curl.body !== null ? 'raw' : 'none';
}

export interface FormRow {
	name: string;
	value: string;
	kind: 'text' | 'file';
	/** `;type=` of a file part. */
	type: string;
	/** Original flag of a text row, kept when it still means the same thing. */
	flag: string;
	/** An entry the table cannot edit (`<file`, `;filename=`), kept as is. */
	raw?: BodyFlag;
}

export interface UrlRow {
	name: string;
	value: string;
	/** `name@file` or plain content, kept as is. */
	raw?: BodyFlag;
}

/** Index of the first `;` that starts a curl form parameter. */
function paramStart(s: string) {
	for (let i = s.indexOf(';'); i !== -1; i = s.indexOf(';', i + 1)) {
		const rest = s.slice(i + 1).trimStart().toLowerCase();
		if (['type=', 'filename=', 'headers=', 'encoder='].some((p) => rest.startsWith(p))) return i;
	}
	return -1;
}

/** Reads `path[;type=x]` or `"path"[;type=x]`; null if anything else is in there. */
function parseFile(rest: string): { path: string; type: string } | null {
	let path: string;
	let tail: string;
	if (rest.startsWith('"')) {
		let out = '';
		let i = 1;
		for (; i < rest.length && rest[i] !== '"'; i++) {
			if (rest[i] === '\\' && i + 1 < rest.length) i++;
			out += rest[i];
		}
		if (i >= rest.length) return null;
		path = out;
		tail = rest.slice(i + 1);
	} else {
		const end = paramStart(rest);
		path = end === -1 ? rest : rest.slice(0, end);
		tail = end === -1 ? '' : rest.slice(end);
	}
	if (!tail) return { path, type: '' };
	const m = /^;\s*type=([^;"]*)$/i.exec(tail);
	return m ? { path, type: m[1] } : null;
}

function formRow(b: BodyFlag): FormRow {
	const raw = { name: '', value: '', kind: 'text' as const, type: '', flag: b.flag, raw: b };
	const eq = b.arg.indexOf('=');
	if (eq === -1) return raw;
	const name = b.arg.slice(0, eq);
	const rest = b.arg.slice(eq + 1);
	if (b.flag === '--form-string') return { name, value: rest, kind: 'text', type: '', flag: b.flag };
	if (rest.startsWith('@')) {
		const file = parseFile(rest.slice(1));
		return file ? { name, value: file.path, kind: 'file', type: file.type, flag: b.flag } : raw;
	}
	if (rest.startsWith('<') || rest.startsWith('"') || paramStart(rest) !== -1) return raw;
	return { name, value: rest, kind: 'text', type: '', flag: b.flag };
}

/** Quotes a file path when `;`, `,` or `"` would change how curl reads it. */
function quotePath(path: string) {
	return /[;,"\\]/.test(path) ? `"${path.replace(/[\\"]/g, '\\$&')}"` : path;
}

function formFlag(r: FormRow): BodyFlag {
	if (r.raw) return r.raw;
	if (r.kind === 'file') {
		const type = r.type.trim() ? `;type=${r.type.trim()}` : '';
		return { flag: '-F', arg: `${r.name}=@${quotePath(r.value)}${type}` };
	}
	// `-F` would read `@x`, `<x`, quotes and `;type=` specially; `--form-string` sends them literally.
	const plain = !/^[@<"]/.test(r.value) && paramStart(r.value) === -1;
	const flag = r.flag !== '--form-string' && plain ? r.flag : '--form-string';
	return { flag, arg: `${r.name}=${r.value}` };
}

export function formRows(flags: string[]): FormRow[] {
	return splitFlags(flags)
		.body.filter((b) => FORM_FLAGS.includes(b.flag))
		.map(formRow);
}

export function urlRows(flags: string[]): UrlRow[] {
	return splitFlags(flags)
		.body.filter((b) => b.flag === URL_FLAG)
		.map((b) => {
			const eq = b.arg.indexOf('=');
			if (eq === -1) return { name: '', value: '', raw: b };
			return { name: b.arg.slice(0, eq), value: b.arg.slice(eq + 1) };
		});
}

/** Replaces every body value flag with `body`, keeping the line flags. */
export function withBodyFlags(flags: string[], body: BodyFlag[]): string[] {
	return joinFlags(splitFlags(flags).line, body);
}

export function setFormRows(flags: string[], rows: FormRow[]): string[] {
	return withBodyFlags(flags, rows.map(formFlag));
}

export function setUrlRows(flags: string[], rows: UrlRow[]): string[] {
	return withBodyFlags(
		flags,
		rows.map((r) => r.raw ?? { flag: URL_FLAG, arg: `${r.name}=${r.value}` })
	);
}

/** Text of every body value, for variable checks. */
export function bodyFlagText(flags: string[]) {
	return splitFlags(flags)
		.body.map((b) => b.arg)
		.join('\n');
}
