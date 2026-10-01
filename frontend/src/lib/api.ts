import type { EnvVar, Info, RequestDoc, RequestForm, RunEvent, TreeNode, TrustStatus } from './types';

const TOKEN_KEY = 'sankh-token';

/** Picks up `#token=...` from the URL once, then keeps it for the tab session. */
function initToken(): string | null {
	const match = location.hash.match(/token=([^&]+)/);
	if (match) {
		sessionStorage.setItem(TOKEN_KEY, decodeURIComponent(match[1]));
		history.replaceState(null, '', location.pathname + location.search);
	}
	return sessionStorage.getItem(TOKEN_KEY);
}

let token = initToken();

export function setToken(value: string) {
	token = value;
	sessionStorage.setItem(TOKEN_KEY, value);
}

export class ApiError extends Error {
	constructor(
		public status: number,
		message: string,
		public details?: unknown
	) {
		super(message);
	}
}

function headers(json: boolean): HeadersInit {
	const h: Record<string, string> = {};
	if (json) h['Content-Type'] = 'application/json';
	if (token) h['Authorization'] = `Bearer ${token}`;
	return h;
}

async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
	const res = await fetch(`/api${path}`, {
		method,
		headers: headers(body !== undefined),
		body: body === undefined ? undefined : JSON.stringify(body)
	});
	const data = await res.json().catch(() => ({}));
	if (!res.ok) throw new ApiError(res.status, data.error ?? res.statusText, data.details);
	return data as T;
}

const enc = (path: string) => path.split('/').map(encodeURIComponent).join('/');

export const api = {
	info: () => call<Info>('GET', '/info'),
	tree: () => call<TreeNode>('GET', '/tree'),
	getRequest: (path: string) => call<RequestDoc>('GET', `/request/${enc(path)}`),
	saveRequest: (path: string, content: string) =>
		call<RequestDoc>('PUT', `/request/${enc(path)}`, { content }),
	deleteRequest: (path: string) => call<unknown>('DELETE', `/request/${enc(path)}`),
	parse: (content: string, path: string) => call<RequestDoc>('POST', '/parse', { content, path }),
	render: (form: RequestForm) => call<{ content: string }>('POST', '/render', { form }),
	importCurl: (curl: string, name: string) =>
		call<{ content: string }>('POST', '/import', { curl, name }),
	envs: () => call<{ envs: string[]; default: string | null }>('GET', '/envs'),
	envVars: (name: string) => call<{ vars: EnvVar[] }>('GET', `/envs/${encodeURIComponent(name || '_')}`),
	clearCaptures: (env: string) =>
		call<unknown>('DELETE', `/captures?env=${encodeURIComponent(env || '_')}`),
	trust: () => call<TrustStatus>('POST', '/trust'),
	startRun: (path: string, env: string) => call<{ id: string }>('POST', '/run', { path, env })
};

/**
 * Reads a server-sent event stream with fetch (EventSource cannot send the
 * Authorization header). Resolves when the stream ends.
 */
export async function streamEvents<T>(path: string, onEvent: (e: T) => void, signal?: AbortSignal) {
	const res = await fetch(`/api${path}`, { headers: headers(false), signal });
	if (!res.ok || !res.body) throw new ApiError(res.status, res.statusText);
	const reader = res.body.pipeThrough(new TextDecoderStream()).getReader();
	let buffer = '';
	for (;;) {
		const { value, done } = await reader.read();
		if (done) break;
		buffer += value;
		let idx;
		while ((idx = buffer.indexOf('\n\n')) >= 0) {
			const chunk = buffer.slice(0, idx);
			buffer = buffer.slice(idx + 2);
			const data = chunk
				.split('\n')
				.filter((l) => l.startsWith('data:'))
				.map((l) => l.slice(5).replace(/^ /, ''))
				.join('\n');
			if (data) {
				try {
					onEvent(JSON.parse(data) as T);
				} catch {
					onEvent(data as T);
				}
			}
		}
	}
}

export async function run(path: string, env: string, onEvent: (e: RunEvent) => void) {
	const { id } = await api.startRun(path, env);
	await streamEvents<RunEvent>(`/runs/${id}/events`, onEvent);
}
