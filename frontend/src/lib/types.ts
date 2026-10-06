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

export type TrustStatus =
	| { state: 'trusted'; path: string }
	| { state: 'untrusted' }
	| {
			state: 'changed';
			path: string;
			trusted_head: string;
			current_head: string;
			changed: string[];
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
	trust: TrustStatus | null;
	default_env: string | null;
}

export interface Info {
	version: string;
	/** False for `sankh serve A B`: workspace changes last for the session only. */
	saved: boolean;
	collections: CollectionInfo[];
}

export interface DirListing {
	path: string;
	parent: string | null;
	collection: boolean;
	dirs: { name: string; path: string; collection: boolean }[];
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

export interface Diagnostic {
	line: number;
	severity: 'warning' | 'error';
	message: string;
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

export interface ResponseView {
	status: number;
	time_ms: number;
	size: number;
	url: string;
	headers: [string, string][];
	body: string;
	body_truncated: boolean;
	body_binary: boolean;
}

export interface RequestResult {
	path: string;
	name: string;
	outcome: 'passed' | 'failed' | 'error' | 'cancelled';
	response: ResponseView | null;
	assertions: AssertionResult[];
	captures: { name: string; value: string }[];
	error: string | null;
	warnings: string[];
	stderr: string;
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
	| { type: 'done'; summary: Summary };

export interface RunState {
	collection: string;
	target: string;
	paths: string[];
	results: Record<string, RequestResult>;
	current: string | null;
	summary: Summary | null;
	/** Paths not run because the run was stopped. */
	skipped: string[];
}

export interface EnvVar {
	name: string;
	value: string;
	source: 'file' | 'capture';
}

/** A variable of one env file as written: `${VAR}` expands, `\$` is literal. */
export interface EnvFileVar {
	name: string;
	value: string;
	/** The name looks secret; the UI masks the value by default. */
	secret?: boolean;
}
