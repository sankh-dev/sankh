import { api } from './api';
import type { EnvVars } from './types';

/**
 * - `ok`: has a real value
 * - `unset`: defined but empty or `changeme`
 * - `missing`: not defined anywhere
 * - `runtime`: not set yet, but a request in the collection captures it
 */
export type VarStatus = 'ok' | 'unset' | 'missing' | 'runtime';

const NAME = /[A-Za-z_][A-Za-z0-9_]*/y;

/** Names referenced as `$VAR` or `${VAR}`, in order, skipping `\$` and single quotes. */
export function refs(text: string): string[] {
	const out: string[] = [];
	for (const r of refSpans(text)) if (!out.includes(r.name)) out.push(r.name);
	return out;
}

/** Each `$VAR` / `${VAR}` reference with its offsets in `text`; `#` comment lines are skipped. */
export function refSpans(text: string): { name: string; from: number; to: number }[] {
	const out: { name: string; from: number; to: number }[] = [];
	let single = false;
	let lineStart = true;
	for (let i = 0; i < text.length; i++) {
		const c = text[i];
		if (lineStart && !single) {
			const ws = /[ \t]*/y;
			ws.lastIndex = i;
			ws.exec(text);
			if (text[ws.lastIndex] === '#') {
				const nl = text.indexOf('\n', i);
				if (nl < 0) break;
				i = nl;
				continue;
			}
		}
		lineStart = c === '\n';
		if (c === "'") single = !single;
		else if (c === '\\') i++;
		else if (c === '$' && !single) {
			const braced = text[i + 1] === '{';
			NAME.lastIndex = i + (braced ? 2 : 1);
			const m = NAME.exec(text);
			if (!m) continue;
			let end = NAME.lastIndex;
			if (braced) {
				if (text[end] !== '}') continue;
				end += 1;
			}
			out.push({ name: m[0], from: i, to: end });
			i = end - 1;
		}
	}
	return out;
}

/** One `$VAR` completion: the name, and a short hint about its value. */
export interface VarOption {
	name: string;
	detail: string;
	status: VarStatus;
}

function preview(value: string) {
	const v = value.replace(/\s+/g, ' ');
	return v.length > 32 ? `${v.slice(0, 31)}…` : v;
}

/** Variables of the selected environment and `.env.local`, plus names requests capture. */
export function varOptions(info: EnvVars | undefined): VarOption[] {
	if (!info) return [];
	const byName = new Map<string, VarOption>();
	for (const v of info.vars) {
		const detail = v.unset ? 'unset' : v.source === 'capture' ? `captured: ${preview(v.value)}` : preview(v.value);
		byName.set(v.name, { name: v.name, detail, status: v.unset ? 'unset' : 'ok' });
	}
	for (const [name, status] of Object.entries(info.refs)) {
		if (status === 'runtime' && !byName.has(name)) {
			byName.set(name, { name, detail: 'set by a capture', status });
		}
	}
	return [...byName.values()].sort((a, b) => a.name.localeCompare(b.name));
}

/** Status lookup over the `/envs/:name` response. */
export function statusFrom(info: EnvVars | undefined): (name: string) => VarStatus {
	if (!info) return () => 'ok';
	const byName = new Map(info.vars.map((v) => [v.name, v]));
	return (name) => {
		const known = info.refs[name];
		if (known) return known;
		const v = byName.get(name);
		if (!v) return 'missing';
		return v.unset ? 'unset' : 'ok';
	};
}

/** Same markers as the server's redaction (`redact.rs`). */
const SECRET = /TOKEN|SECRET|KEY|PASSWORD|PASSWD|AUTH|COOKIE/i;

export function isSecretName(name: string) {
	return SECRET.test(name);
}

/** Empty, or the import placeholder; mirrors `env::is_placeholder`. */
export function isPlaceholder(value: string) {
	const v = value.trim();
	return v === '' || v.toLowerCase() === 'changeme';
}

export function needsValue(s: VarStatus) {
	return s === 'unset' || s === 'missing';
}

export const STATUS_TEXT: Record<VarStatus, string> = {
	ok: 'set',
	unset: 'empty or placeholder',
	missing: 'not defined',
	runtime: 'set by a capture when the collection runs'
};

/** Sets `name` in the collection's `.env.local`, keeping its other lines. */
export async function setLocalVar(cid: string, name: string, value: string) {
	const { vars } = await api.envLocal(cid);
	const i = vars.findIndex((v) => v.name === name);
	if (i >= 0) vars[i] = { name, value };
	else vars.push({ name, value });
	await api.saveEnvLocal(cid, vars.map(({ name, value }) => ({ name, value })));
}
