<script lang="ts">
	import { onMount } from 'svelte';
	import CollectionSection from './lib/CollectionSection.svelte';
	import TrustBanner from './lib/TrustBanner.svelte';
	import RequestEditor from './lib/RequestEditor.svelte';
	import ResponsePane from './lib/ResponsePane.svelte';
	import RunPanel from './lib/RunPanel.svelte';
	import NewRequestDialog from './lib/NewRequestDialog.svelte';
	import AddFolderDialog from './lib/AddFolderDialog.svelte';
	import CopyDialog from './lib/CopyDialog.svelte';
	import { ApiError, api, run, setToken, streamEvents } from './lib/api';
	import type { CollectionInfo, Info, RequestDoc, RequestResult, RunState, TreeNode } from './lib/types';

	/** A request (or folder) inside a collection. */
	type Ref = { cid: string; path: string };

	const ENV_KEY = (cid: string) => `sankh-env:${cid}`;
	const key = (r: Ref) => `${r.cid}:${r.path}`;

	let info = $state.raw<Info | null>(null);
	let trees = $state.raw<Record<string, TreeNode>>({});
	let envLists = $state.raw<Record<string, string[]>>({});
	let envs = $state<Record<string, string>>({});
	let vars = $state.raw<Record<string, string[]>>({});
	let selected = $state.raw<Ref | null>(null);
	let doc = $state.raw<RequestDoc | null>(null);
	let docRef = $state.raw<Ref | null>(null);
	let docVersion = $state(0);
	let results = $state<Record<string, RequestResult>>({});
	let runState = $state<RunState | null>(null);
	let running = $state.raw<Ref | null>(null);
	let newIn = $state.raw<Ref | null>(null);
	let adding = $state(false);
	let copying = $state(false);
	let error = $state<string | null>(null);
	let needToken = $state(false);
	let tokenInput = $state('');
	let editor = $state<ReturnType<typeof RequestEditor>>();

	let collections = $derived(info?.collections ?? []);
	let byId = $derived(Object.fromEntries(collections.map((c) => [c.id, c])) as Record<string, CollectionInfo>);
	let active = $derived(
		(selected && byId[selected.cid]) || collections.find((c) => !c.missing) || null
	);
	let onlyScratch = $derived(collections.every((c) => c.scratch));
	let outcomes = $derived.by(() => {
		const out: Record<string, Record<string, string>> = {};
		for (const [k, r] of Object.entries(results)) {
			const i = k.indexOf(':');
			(out[k.slice(0, i)] ??= {})[k.slice(i + 1)] = r.outcome;
		}
		return out;
	});
	let shownResult = $derived(selected ? (results[key(selected)] ?? null) : null);
	let copyTargets = $derived(collections.filter((c) => !c.missing && c.id !== docRef?.cid));

	function isTrusted(cid: string) {
		return byId[cid]?.trust?.state === 'trusted';
	}

	function fail(e: unknown) {
		if (e instanceof ApiError && e.status === 401) {
			needToken = true;
			return;
		}
		error = e instanceof Error ? e.message : String(e);
	}

	async function load() {
		try {
			info = await api.info();
			needToken = false;
			await Promise.all(info.collections.filter((c) => !c.missing).map(loadCollection));
		} catch (e) {
			fail(e);
		}
	}

	async function loadCollection(c: CollectionInfo) {
		try {
			const [t, e] = await Promise.all([api.tree(c.id), api.envs(c.id)]);
			trees = { ...trees, [c.id]: t };
			envLists = { ...envLists, [c.id]: e.envs };
			const saved = localStorage.getItem(ENV_KEY(c.id));
			envs[c.id] =
				saved !== null && (saved === '' || e.envs.includes(saved)) ? saved : (e.default ?? e.envs[0] ?? '');
			await loadVars(c.id);
		} catch (e) {
			fail(e);
		}
	}

	async function loadVars(cid: string) {
		let names: string[] = [];
		try {
			names = (await api.envVars(cid, envs[cid] ?? '')).vars.map((v) => v.name);
		} catch {
			// unknown env or missing folder: no completions
		}
		vars = { ...vars, [cid]: names };
	}

	async function refreshTree(cid: string) {
		if (!byId[cid] || byId[cid].missing) return;
		try {
			trees = { ...trees, [cid]: await api.tree(cid) };
		} catch (e) {
			fail(e);
		}
	}

	async function open(cid: string, path: string) {
		selected = { cid, path };
		try {
			doc = await api.getRequest(cid, path);
			docRef = { cid, path };
			docVersion++;
		} catch (e) {
			fail(e);
		}
	}

	async function save(content: string): Promise<RequestDoc> {
		const ref = docRef!;
		const saved = await api.saveRequest(ref.cid, doc!.path, content);
		await refreshTree(ref.cid);
		return saved;
	}

	async function remove() {
		if (!doc || !docRef || !confirm(`Delete ${doc.path}?`)) return;
		const ref = docRef;
		try {
			await api.deleteRequest(ref.cid, doc.path);
			doc = null;
			docRef = null;
			selected = null;
			await refreshTree(ref.cid);
		} catch (e) {
			fail(e);
		}
	}

	async function create(path: string, content: string) {
		const cid = newIn!.cid;
		await api.saveRequest(cid, path, content);
		newIn = null;
		await refreshTree(cid);
		await open(cid, path);
	}

	async function copyTo(to: string, toPath: string) {
		const from = docRef!;
		const copied = await api.copyRequest(from.cid, from.path, to, toPath);
		copying = false;
		await refreshTree(to);
		await open(to, copied.path);
	}

	async function runPath(cid: string, path: string) {
		if (!isTrusted(cid) || running !== null) return;
		error = null;
		const isFile = path.endsWith('.sh');
		runState = isFile
			? null
			: { collection: cid, target: path, paths: [], results: {}, current: null, summary: null };
		running = { cid, path };
		try {
			await run(cid, path, envs[cid] ?? '', (ev) => {
				if (ev.type === 'start' && runState) runState.paths = ev.paths;
				else if (ev.type === 'running') {
					running = { cid, path: ev.path };
					if (runState) runState.current = ev.path;
				} else if (ev.type === 'result') {
					results[key({ cid, path: ev.result.path })] = ev.result;
					if (runState) runState.results[ev.result.path] = ev.result;
					if (isFile) selected = { cid, path: ev.result.path };
				} else if (ev.type === 'done' && runState) {
					runState.summary = ev.summary;
					runState.current = null;
				}
			});
			await loadVars(cid);
		} catch (e) {
			if (e instanceof ApiError && e.status === 403) info = await api.info();
			fail(e);
		} finally {
			running = null;
		}
	}

	async function trust(cid: string) {
		try {
			await api.trust(cid);
			info = await api.info();
		} catch (e) {
			fail(e);
		}
	}

	async function changeEnv(cid: string, value: string) {
		envs[cid] = value;
		localStorage.setItem(ENV_KEY(cid), value);
		await loadVars(cid);
	}

	async function clearCaptures() {
		if (!active) return;
		const cid = active.id;
		try {
			await api.clearCaptures(cid, envs[cid] ?? '');
			await loadVars(cid);
		} catch (e) {
			fail(e);
		}
	}

	async function addFolder(path: string) {
		const col = await api.addCollection(path);
		adding = false;
		info = await api.info();
		await loadCollection(col);
	}

	async function unlink(cid: string) {
		const col = byId[cid];
		if (!col || !confirm(`Unlink "${col.name}" from the workspace?\n\nIts files stay on disk at ${col.root}.`)) return;
		try {
			await api.removeCollection(cid);
			info = await api.info();
			const drop = <T,>(m: Record<string, T>) => Object.fromEntries(Object.entries(m).filter(([k]) => k !== cid));
			trees = drop(trees);
			envLists = drop(envLists);
			vars = drop(vars);
			for (const k of Object.keys(results)) if (k.startsWith(`${cid}:`)) delete results[k];
			if (selected?.cid === cid) selected = null;
			if (docRef?.cid === cid) {
				doc = null;
				docRef = null;
			}
			if (runState?.collection === cid) runState = null;
		} catch (e) {
			fail(e);
		}
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
		const onChange = (ev: { changed?: string | null } | string) => {
			const cid = typeof ev === 'object' ? ev.changed : null;
			if (cid) refreshTree(cid);
			else for (const c of collections) refreshTree(c.id);
		};
		const watch = async () => {
			while (!controller.signal.aborted) {
				try {
					await streamEvents('/events', onChange, controller.signal);
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
		{#if active}
			<span class="collection" title={active.root}>
				{active.name}
				<span class="muted">· env {envs[active.id] || 'none'}</span>
			</span>
		{/if}
		<span class="spacer"></span>
		<button
			onclick={clearCaptures}
			disabled={!active}
			title="Forget values captured in this session for {active?.name ?? 'the collection'} ({active ? envs[active.id] || 'no env' : ''})"
		>
			Clear captures
		</button>
		<button onclick={() => active && (newIn = { cid: active.id, path: '' })} disabled={!active || active.missing}>
			New request
		</button>
		<button onclick={() => (adding = true)}>Add folder</button>
	</header>

	{#if active && active.trust && !active.missing}
		<TrustBanner name={active.name} trust={active.trust} root={active.root} ontrust={() => trust(active!.id)} />
	{/if}
	{#if error}
		<div class="error-bar" role="alert">
			<span>{error}</span>
			<button onclick={() => (error = null)}>Dismiss</button>
		</div>
	{/if}

	<main>
		<aside>
			{#each collections as col (col.id)}
				<CollectionSection
					{col}
					tree={trees[col.id] ?? null}
					envs={envLists[col.id] ?? []}
					env={envs[col.id] ?? ''}
					selected={selected?.cid === col.id ? selected.path : null}
					running={running?.cid === col.id ? running.path : null}
					outcomes={outcomes[col.id] ?? {}}
					busy={running !== null}
					onselect={(p) => open(col.id, p)}
					onrun={(p) => runPath(col.id, p)}
					onnew={(f) => (newIn = { cid: col.id, path: f })}
					onenv={(e) => changeEnv(col.id, e)}
					onunlink={() => unlink(col.id)}
				/>
			{/each}
			{#if info && onlyScratch}
				<p class="hint">Add a folder, or try requests in Scratch.</p>
			{/if}
			<button class="add" onclick={() => (adding = true)}>+ Add folder</button>
			{#if info && !info.saved}
				<p class="hint">Session only: changes to this list are not saved.</p>
			{/if}
		</aside>

		<section class="editor-col">
			{#if doc && docRef}
				{#key `${docRef.cid}:${doc.path}:${docVersion}`}
					<RequestEditor
						bind:this={editor}
						cid={docRef.cid}
						collection={byId[docRef.cid]?.name ?? docRef.cid}
						{doc}
						vars={vars[docRef.cid] ?? []}
						canRun={isTrusted(docRef.cid)}
						running={running !== null}
						onsave={save}
						onrun={() => runPath(docRef!.cid, doc!.path)}
						ondelete={remove}
						oncopy={copyTargets.length ? () => (copying = true) : undefined}
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
				<RunPanel
					run={runState}
					name={byId[runState.collection]?.name ?? runState.collection}
					selected={selected?.cid === runState.collection ? selected.path : null}
					onpick={(p) => open(runState!.collection, p)}
				/>
			{/if}
			<ResponsePane result={shownResult} />
		</section>
	</main>
</div>

{#if newIn !== null}
	<NewRequestDialog
		collection={byId[newIn.cid]?.name ?? newIn.cid}
		folder={newIn.path}
		oncreate={create}
		onclose={() => (newIn = null)}
	/>
{/if}

{#if adding}
	<AddFolderDialog saved={info?.saved ?? true} onadd={addFolder} onclose={() => (adding = false)} />
{/if}

{#if copying && docRef}
	<CopyDialog path={docRef.path} targets={copyTargets} oncopy={copyTo} onclose={() => (copying = false)} />
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
		padding-left: 10px;
		border-left: 1px solid var(--border);
	}
	.spacer {
		flex: 1;
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
		padding: 0 0 6px;
		background: var(--panel);
	}
	.add {
		display: block;
		margin: 8px 10px 4px;
		width: calc(100% - 20px);
		background: none;
		border-style: dashed;
		color: var(--muted);
	}
	.hint {
		margin: 6px 12px;
		color: var(--muted);
		font-size: 12px;
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
