import type {
	CollectionInfo,
	DirListing,
	EnvVar,
	Info,
	PostmanImported,
	PostmanImportRequest,
	PostmanPreview,
	RequestDoc,
	RequestForm,
	RunEvent,
	TreeNode,
	TrustStatus
} from './types';

const TOKEN_KEY = 'sankh-token';

/**
 * Where the API lives. The web UI is served by the API itself, so `/api`;
 * an embedding desktop app can set `window.__SANKH_API_BASE__` before load.
 */
let apiBase = (globalThis as { __SANKH_API_BASE__?: string }).__SANKH_API_BASE__ ?? '/api';

export function setApiBase(base: string) {
	apiBase = base.replace(/\/$/, '');
}

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
	const res = await fetch(`${apiBase}${path}`, {
		method,
		headers: headers(body !== undefined),
		body: body === undefined ? undefined : JSON.stringify(body)
	});
	const data = await res.json().catch(() => ({}));
	if (!res.ok) throw new ApiError(res.status, data.error ?? res.statusText, data.details);
	return data as T;
}

const enc = (path: string) => path.split('/').map(encodeURIComponent).join('/');
const c = (cid: string) => `/c/${encodeURIComponent(cid)}`;

export const api = {
	info: () => call<Info>('GET', '/info'),
	addCollection: (path: string) => call<CollectionInfo>('POST', '/collections', { path }),
	removeCollection: (cid: string) => call<unknown>('DELETE', `/collections/${encodeURIComponent(cid)}`),
	listDirs: (path?: string) =>
		call<DirListing>('GET', `/fs/dirs${path ? `?path=${encodeURIComponent(path)}` : ''}`),
	render: (form: RequestForm) => call<{ content: string }>('POST', '/render', { form }),
	importCurl: (curl: string, name: string) =>
		call<{ content: string }>('POST', '/import', { curl, name }),
	previewPostman: (body: PostmanImportRequest) =>
		call<PostmanPreview>('POST', '/import/postman', { ...body, write: false }),
	importPostman: (body: PostmanImportRequest) =>
		call<PostmanImported>('POST', '/import/postman', { ...body, write: true }),

	tree: (cid: string) => call<TreeNode>('GET', `${c(cid)}/tree`),
	getRequest: (cid: string, path: string) => call<RequestDoc>('GET', `${c(cid)}/request/${enc(path)}`),
	saveRequest: (cid: string, path: string, content: string) =>
		call<RequestDoc>('PUT', `${c(cid)}/request/${enc(path)}`, { content }),
	deleteRequest: (cid: string, path: string) => call<unknown>('DELETE', `${c(cid)}/request/${enc(path)}`),
	copyRequest: (cid: string, path: string, to: string, toPath: string) =>
		call<{ collection: string; path: string }>('POST', `${c(cid)}/copy`, { path, to, to_path: toPath }),
	parse: (cid: string, content: string, path: string) =>
		call<RequestDoc>('POST', `${c(cid)}/parse`, { content, path }),
	envs: (cid: string) => call<{ envs: string[]; default: string | null }>('GET', `${c(cid)}/envs`),
	envVars: (cid: string, name: string) =>
		call<{ vars: EnvVar[] }>('GET', `${c(cid)}/envs/${encodeURIComponent(name || '_')}`),
	clearCaptures: (cid: string, env: string) =>
		call<unknown>('DELETE', `${c(cid)}/captures?env=${encodeURIComponent(env || '_')}`),
	trust: (cid: string) => call<TrustStatus>('POST', `${c(cid)}/trust`),
	startRun: (cid: string, path: string, env: string) =>
		call<{ id: string }>('POST', `${c(cid)}/run`, { path, env })
};

/**
 * Reads a server-sent event stream with fetch (EventSource cannot send the
 * Authorization header). Resolves when the stream ends.
 */
export async function streamEvents<T>(path: string, onEvent: (e: T) => void, signal?: AbortSignal) {
	const res = await fetch(`${apiBase}${path}`, { headers: headers(false), signal });
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

export async function run(cid: string, path: string, env: string, onEvent: (e: RunEvent) => void) {
	const { id } = await api.startRun(cid, path, env);
	await streamEvents<RunEvent>(`/runs/${id}/events`, onEvent);
}
