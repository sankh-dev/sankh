<script lang="ts">
	import { onMount } from 'svelte';
	import Tree from './lib/Tree.svelte';
	import TrustBanner from './lib/TrustBanner.svelte';
	import RequestEditor from './lib/RequestEditor.svelte';
	import ResponsePane from './lib/ResponsePane.svelte';
	import RunPanel from './lib/RunPanel.svelte';
	import NewRequestDialog from './lib/NewRequestDialog.svelte';
	import { ApiError, api, run, setToken, streamEvents } from './lib/api';
	import type { Info, RequestDoc, RequestResult, RunState, TreeNode } from './lib/types';

	const ENV_KEY = 'sankh-env';

	let info = $state.raw<Info | null>(null);
	let tree = $state.raw<TreeNode | null>(null);
	let envs = $state.raw<string[]>([]);
	let env = $state('');
	let vars = $state.raw<string[]>([]);
	let selected = $state<string | null>(null);
	let doc = $state.raw<RequestDoc | null>(null);
	let docVersion = $state(0);
	let results = $state<Record<string, RequestResult>>({});
	let runState = $state<RunState | null>(null);
	let running = $state<string | null>(null);
	let newIn = $state<string | null>(null);
	let error = $state<string | null>(null);
	let needToken = $state(false);
	let tokenInput = $state('');
	let editor = $state<ReturnType<typeof RequestEditor>>();

	let trusted = $derived(info?.trust.state === 'trusted');
	let outcomes = $derived(Object.fromEntries(Object.entries(results).map(([p, r]) => [p, r.outcome])));
	let shownResult = $derived(selected ? (results[selected] ?? null) : null);

	function fail(e: unknown) {
		if (e instanceof ApiError && e.status === 401) {
			needToken = true;
			return;
		}
		error = e instanceof Error ? e.message : String(e);
	}

	async function load() {
		try {
			const [i, t, e] = await Promise.all([api.info(), api.tree(), api.envs()]);
			info = i;
			tree = t;
			envs = e.envs;
			const saved = localStorage.getItem(ENV_KEY);
			env = saved !== null && (saved === '' || e.envs.includes(saved)) ? saved : (e.default ?? e.envs[0] ?? '');
			needToken = false;
			await loadVars();
		} catch (e) {
			fail(e);
		}
	}

	async function loadVars() {
		try {
			const res = await api.envVars(env);
			vars = res.vars.map((v) => v.name);
		} catch {
			vars = [];
		}
	}

	async function refreshTree() {
		try {
			tree = await api.tree();
		} catch (e) {
			fail(e);
		}
	}

	async function open(path: string) {
		selected = path;
		try {
			doc = await api.getRequest(path);
			docVersion++;
		} catch (e) {
			fail(e);
		}
	}

	async function save(content: string): Promise<RequestDoc> {
		const saved = await api.saveRequest(doc!.path, content);
		await refreshTree();
		return saved;
	}

	async function remove() {
		if (!doc || !confirm(`Delete ${doc.path}?`)) return;
		try {
			await api.deleteRequest(doc.path);
			doc = null;
			selected = null;
			await refreshTree();
		} catch (e) {
			fail(e);
		}
	}

	async function create(path: string, content: string) {
		await api.saveRequest(path, content);
		newIn = null;
		await refreshTree();
		await open(path);
	}

	async function runPath(path: string) {
		if (!trusted || running !== null) return;
		error = null;
		const isFile = path.endsWith('.sh');
		runState = isFile ? null : { target: path, paths: [], results: {}, current: null, summary: null };
		running = path;
		try {
			await run(path, env, (ev) => {
				if (ev.type === 'start' && runState) runState.paths = ev.paths;
				else if (ev.type === 'running') {
					running = ev.path;
					if (runState) runState.current = ev.path;
				} else if (ev.type === 'result') {
					results[ev.result.path] = ev.result;
					if (runState) runState.results[ev.result.path] = ev.result;
					if (isFile) selected = ev.result.path;
				} else if (ev.type === 'done' && runState) {
					runState.summary = ev.summary;
					runState.current = null;
				}
			});
			await loadVars();
		} catch (e) {
			if (e instanceof ApiError && e.status === 403) info = await api.info();
			fail(e);
		} finally {
			running = null;
		}
	}

	async function trust() {
		try {
			await api.trust();
			info = await api.info();
		} catch (e) {
			fail(e);
		}
	}

	async function changeEnv() {
		localStorage.setItem(ENV_KEY, env);
		await loadVars();
	}

	async function clearCaptures() {
		await api.clearCaptures(env);
		await loadVars();
	}

	function submitToken(event: SubmitEvent) {
		event.preventDefault();
		setToken(tokenInput.trim());
		load();
	}

	function onkeydown(e: KeyboardEvent) {
		if (!(e.ctrlKey || e.metaKey)) return;
		if (e.key === 's' && editor) {
			e.preventDefault();
			editor.save();
		} else if (e.key === 'Enter' && editor) {
			e.preventDefault();
			editor.saveAndRun();
		}
	}

	onMount(() => {
		load();
		const controller = new AbortController();
		const watch = async () => {
			while (!controller.signal.aborted) {
				try {
					await streamEvents('/events', () => refreshTree(), controller.signal);
				} catch {
					// reconnect below
				}
				await new Promise((r) => setTimeout(r, 2000));
			}
		};
		watch();
		return () => controller.abort();
	});
</script>

<svelte:window {onkeydown} />

<div class="app">
	<header>
		<img src="/logo.svg" alt="" width="26" height="26" />
		<span class="brand">sankh</span>
		{#if info}<span class="collection">{info.name}</span>{/if}
		<span class="spacer"></span>
		<label class="env">
			env
			<select bind:value={env} onchange={changeEnv}>
				<option value="">(none)</option>
				{#each envs as e (e)}
					<option value={e}>{e}</option>
				{/each}
			</select>
		</label>
		<button onclick={clearCaptures} title="Forget values captured in this session">Clear captures</button>
		<button onclick={() => (newIn = '')}>New request</button>
		<button class="primary" disabled={!trusted || running !== null} onclick={() => runPath('')}>
			Run all
		</button>
	</header>

	{#if info}
		<TrustBanner trust={info.trust} root={info.root} ontrust={trust} />
	{/if}
	{#if error}
		<div class="error-bar" role="alert">
			<span>{error}</span>
			<button onclick={() => (error = null)}>Dismiss</button>
		</div>
	{/if}

	<main>
		<aside>
			{#if tree}
				<Tree node={tree} {selected} {running} {outcomes} onselect={open} onrun={runPath} onnew={(f) => (newIn = f)} />
			{/if}
		</aside>

		<section class="editor-col">
			{#if doc}
				{#key `${doc.path}:${docVersion}`}
					<RequestEditor
						bind:this={editor}
						{doc}
						{vars}
						canRun={trusted}
						running={running !== null}
						onsave={save}
						onrun={() => runPath(doc!.path)}
						ondelete={remove}
					/>
				{/key}
			{:else}
				<div class="placeholder">
					<img src="/logo.svg" alt="" width="64" height="64" />
					<p>Pick a request on the left, or create one.</p>
					<p class="muted">Ctrl+S saves · Ctrl+Enter runs</p>
				</div>
			{/if}
		</section>

		<section class="response-col">
			{#if runState}
				<RunPanel run={runState} {selected} onpick={open} />
			{/if}
			<ResponsePane result={shownResult} />
		</section>
	</main>
</div>

{#if newIn !== null}
	<NewRequestDialog folder={newIn} oncreate={create} onclose={() => (newIn = null)} />
{/if}

{#if needToken}
	<div class="backdrop">
		<form class="token" onsubmit={submitToken}>
			<h3>Token required</h3>
			<p class="muted">This server was started with <code>--token</code>.</p>
			<input type="password" bind:value={tokenInput} placeholder="token" aria-label="Token" />
			<button class="primary" type="submit">Connect</button>
		</form>
	</div>
{/if}

<style>
	.app {
		display: flex;
		flex-direction: column;
		height: 100%;
	}
	header {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 14px;
		border-bottom: 1px solid var(--border);
		background: var(--panel);
	}
	.brand {
		font-weight: 700;
		font-size: 15px;
		letter-spacing: 0.02em;
	}
	.collection {
		color: var(--muted);
		padding-left: 10px;
		border-left: 1px solid var(--border);
	}
	.spacer {
		flex: 1;
	}
	.env {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--muted);
	}
	main {
		flex: 1;
		display: grid;
		grid-template-columns: 270px minmax(0, 1fr) minmax(0, 1fr);
		min-height: 0;
	}
	aside {
		border-right: 1px solid var(--border);
		overflow: auto;
		padding: 6px 0;
		background: var(--panel);
	}
	.editor-col {
		border-right: 1px solid var(--border);
		min-height: 0;
	}
	.response-col {
		display: flex;
		flex-direction: column;
		min-height: 0;
	}
	.placeholder {
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 4px;
		opacity: 0.85;
	}
	.muted {
		color: var(--muted);
	}
	.error-bar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 6px 14px;
		background: #4a1c1c;
		color: #ffd0d0;
		white-space: pre-wrap;
	}
	.backdrop {
		position: fixed;
		inset: 0;
		background: #000a;
		display: grid;
		place-items: center;
	}
	.token {
		background: var(--panel);
		border: 1px solid var(--border);
		border-radius: 10px;
		padding: 18px;
		display: flex;
		flex-direction: column;
		gap: 10px;
		width: 320px;
	}
	.token h3 {
		margin: 0;
	}
	@media (max-width: 1100px) {
		main {
			grid-template-columns: 220px minmax(0, 1fr);
			grid-template-rows: minmax(0, 1fr) minmax(0, 1fr);
		}
		aside {
			grid-row: span 2;
		}
		.editor-col {
			border-right: none;
			border-bottom: 1px solid var(--border);
		}
	}
</style>
