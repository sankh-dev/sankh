export type TreeNode =
	| { type: 'folder'; name: string; path: string; children: TreeNode[] }
	| {
			type: 'request';
			name: string;
			path: string;
			tags: string[];
			method: string | null;
			raw: boolean;
			errors: number;
	  };

export interface CollectionInfo {
	id: string;
	name: string;
	root: string;
	/** The built-in Scratch collection, which cannot be unlinked. */
	scratch: boolean;
	/** The folder is gone or cannot be opened; `error` says why. */
	missing: boolean;
	error: string | null;
	default_env: string | null;
}

export interface Info {
	version: string;
	/** False for `sankh serve A B`: workspace changes last for the session only. */
	saved: boolean;
	/** The server runs on this machine, so it can open folders in the file manager. */
	can_reveal: boolean;
	collections: CollectionInfo[];
}

export interface DirListing {
	path: string;
	parent: string | null;
	collection: boolean;
	dirs: { name: string; path: string; collection: boolean }[];
}

/** One folder inside a collection; paths are relative to the collection root. */
export interface FileListing {
	dir: string;
	/** `""` is the root; null when `dir` is the root. */
	parent: string | null;
	dirs: { name: string; path: string }[];
	files: { name: string; path: string; size: number }[];
}

export interface ImportReport {
	collection: string;
	requests: number;
	folders: number;
	environments: string[];
	renamed: { from: string; to: string }[];
	placeholders: { name: string; note: string }[];
	/** `path` is the generated file or folder; `""` for the collection. */
	warnings: { path: string; message: string }[];
}

export interface PostmanImportRequest {
	collection: string;
	envs: string[];
	dir?: string;
	force?: boolean;
	write?: boolean;
}

export interface PostmanPreview {
	dir: string;
	/** The output folder exists and has files in it. */
	nonempty: boolean;
	files: string[];
	report: ImportReport;
}

export interface PostmanImported {
	dir: string;
	collection: CollectionInfo;
	report: ImportReport;
}

export interface Header {
	name: string;
	value: string;
}

export interface CurlCommand {
	method: string;
	url: string;
	headers: Header[];
	body: string | null;
	body_flag: string | null;
	flags: string[];
}

export type ErrorKind = 'assertion' | 'transport' | 'parse' | 'config';

export interface Diagnostic {
	/** 0 is the whole file. */
	line: number;
	severity: 'warning' | 'error';
	message: string;
	kind?: ErrorKind;
	/** 1-based character column. */
	column?: number;
	/** Characters to underline from `column`. */
	width?: number;
	label?: string;
	help?: string;
	note?: string;
}

export interface ParsedRequest {
	name: string;
	description: string | null;
	tags: string[];
	timeout_ms?: number;
	mode: 'form' | 'raw';
	raw_reason: string | null;
	curl: CurlCommand | null;
	body: string;
	diagnostics: Diagnostic[];
}

export interface RequestForm {
	shebang: string | null;
	name: string;
	description: string | null;
	tags: string[];
	/** `@timeout` argument, e.g. `10s`; null uses the collection default. */
	timeout: string | null;
	expects: string[];
	captures: string[];
	extra_header_lines: string[];
	curl: CurlCommand;
}

export interface RequestDoc {
	path: string;
	content: string;
	request: ParsedRequest;
	form: RequestForm | null;
}

export interface AssertionResult {
	label: string;
	passed: boolean;
	message: string | null;
}

/** What was sent. */
export interface RequestView {
	method: string;
	url: string;
	headers: [string, string][];
	body?: string;
	body_size: number;
}

export interface RedirectHop {
	status: number;
	url: string;
	location: string;
}

export interface ResponseView {
	status: number;
	/** `HTTP/1.1`, `HTTP/2.0`. */
	version: string;
	time_ms: number;
	/** Until the response headers arrived. */
	ttfb_ms?: number;
	size: number;
	url: string;
	headers: [string, string][];
	body: string;
	/** Only part of the body is shown (see `body_capped`). */
	body_truncated: boolean;
	/** The body was longer than `max_body` and was not read in full. */
	body_capped: boolean;
	body_binary: boolean;
	/** SHA-256 of a binary body, hex. */
	sha256?: string;
	body_base64?: string;
	redirects?: RedirectHop[];
}

export interface RequestResult {
	path: string;
	name: string;
	outcome: 'passed' | 'failed' | 'error' | 'cancelled';
	/** Why an `error` / `failed` outcome happened. */
	error_kind?: ErrorKind;
	request?: RequestView;
	response: ResponseView | null;
	assertions: AssertionResult[];
	captures: { name: string; value: string }[];
	error: string | null;
	warnings: string[];
	diagnostics?: Diagnostic[];
	/** TLS certificate checks were skipped (`-k`). */
	insecure?: boolean;
	duration_ms: number;
}

export interface Summary {
	total: number;
	passed: number;
	failed: number;
	errors: number;
	cancelled: number;
	duration_ms: number;
}

export type RunEvent =
	| { type: 'start'; collection: string; total: number; paths: string[] }
	| { type: 'running'; path: string }
	| { type: 'result'; result: RequestResult }
	| { type: 'cancelled'; skipped: string[] }
	/** Some files have errors: only their results are sent, nothing ran. */
	| { type: 'blocked'; skipped: string[] }
	| { type: 'done'; summary: Summary };

export interface RunState {
	collection: string;
	target: string;
	paths: string[];
	results: Record<string, RequestResult>;
	current: string | null;
	summary: Summary | null;
	/** Paths not run because the run was stopped or blocked. */
	skipped: string[];
	/** Files with errors stopped the run before anything was sent. */
	blocked: boolean;
}

export interface EnvVar {
	name: string;
	value: string;
	/** `file` is the environment file, `local` is `.env.local`. */
	source: 'file' | 'local' | 'capture';
	/** Empty or the import placeholder (`changeme`), and not set elsewhere. */
	unset: boolean;
}

export interface EnvVars {
	vars: EnvVar[];
	/** Status of every variable the collection's requests reference. */
	refs: Record<string, 'ok' | 'unset' | 'missing' | 'runtime'>;
}

/** What importing one Postman environment does to its file. */
export interface EnvFilePlan {
	/** Name in the Postman export. */
	source: string;
	env: string;
	file: string;
	status: 'new' | 'merge';
	added: string[];
	updated: string[];
	unchanged: number;
}

export interface EnvImportPlan {
	envs: EnvFilePlan[];
	renamed: { from: string; to: string }[];
	/** Secret variables, kept out of the env files. */
	secrets: string[];
	local_added: string[];
	local_updated: string[];
}

/** A variable of one env file as written: `${VAR}` expands, `\$` is literal. */
export interface EnvFileVar {
	name: string;
	value: string;
	/** The name looks secret; the UI masks the value by default. */
	secret?: boolean;
}
