<script lang="ts">
	import { onMount } from 'svelte';
	import CollectionSection from './lib/CollectionSection.svelte';
	import RequestEditor from './lib/RequestEditor.svelte';
	import ResponsePane from './lib/ResponsePane.svelte';
	import RunPanel from './lib/RunPanel.svelte';
	import NewRequestDialog from './lib/NewRequestDialog.svelte';
	import AddFolderDialog from './lib/AddFolderDialog.svelte';
	import ImportPostmanDialog from './lib/ImportPostmanDialog.svelte';
	import ImportPostmanEnvDialog from './lib/ImportPostmanEnvDialog.svelte';
	import CopyDialog from './lib/CopyDialog.svelte';
	import EnvManager, { type EnvChange } from './lib/EnvManager.svelte';
	import Icon from './lib/Icon.svelte';
	import Menu, { type MenuItem } from './lib/Menu.svelte';
	import SecretInput from './lib/SecretInput.svelte';
	import SetVarPopover from './lib/SetVarPopover.svelte';
	import Splitter from './lib/Splitter.svelte';
	import { ApiError, api, run, setToken, streamEvents } from './lib/api';
	import { infoFrom, needsValue, statusFrom, varOptions } from './lib/vars';
	import { anyCollapsed, collapseAll, expandAll, reveal } from './lib/folders.svelte';
	import type { CollectionInfo, EnvVars, Info, RequestDoc, RequestResult, RunState, TreeNode } from './lib/types';

	/** A request (or folder) inside a collection. */
	type Ref = { cid: string; path: string };

	const ENV_KEY = (cid: string) => `sankh-env:${cid}`;
	const key = (r: Ref) => `${r.cid}:${r.path}`;

	const SIDEBAR_KEY = 'sankh-layout:sidebar';
	const EDITOR_KEY = 'sankh-layout:editor';
	const SIDEBAR_DEFAULT = 270;
	const SIDEBAR_MIN = 180;
	const SIDEBAR_MAX = 600;
	const COL_MIN = 280;
	const SPLITTER_W = 4;

	function stored(k: string, fallback: number, ok: (n: number) => boolean) {
		const n = Number(localStorage.getItem(k));
		return localStorage.getItem(k) !== null && Number.isFinite(n) && ok(n) ? n : fallback;
	}

	let sidebarW = $state(stored(SIDEBAR_KEY, SIDEBAR_DEFAULT, (n) => n >= SIDEBAR_MIN && n <= SIDEBAR_MAX));
	/** Editor's share of the width left after the sidebar. */
	let editorFrac = $state(stored(EDITOR_KEY, 0.5, (n) => n > 0 && n < 1));
	let mainW = $state(0);
	let restW = $derived(Math.max(0, mainW - sidebarW - 2 * SPLITTER_W));

	$effect(() => localStorage.setItem(SIDEBAR_KEY, String(sidebarW)));
	$effect(() => localStorage.setItem(EDITOR_KEY, String(editorFrac)));

	let info = $state.raw<Info | null>(null);
	let trees = $state.raw<Record<string, TreeNode>>({});
	let envLists = $state.raw<Record<string, string[]>>({});
	let envs = $state<Record<string, string>>({});
	let envVars = $state.raw<Record<string, EnvVars>>({});
	let selected = $state.raw<Ref | null>(null);
	let doc = $state.raw<RequestDoc | null>(null);
	let docRef = $state.raw<Ref | null>(null);
	let docVersion = $state(0);
	let results = $state<Record<string, RequestResult>>({});
	let runState = $state<RunState | null>(null);
	let running = $state.raw<Ref | null>(null);
	let runId = $state<string | null>(null);
	let stopping = $state(false);
	let newIn = $state.raw<Ref | null>(null);
	let adding = $state(false);
	let importing = $state(false);
	let copying = $state(false);
	let managingEnvs = $state<string | null>(null);
	/** Collection whose Postman environment import is open; `back` reopens the environment manager. */
	let importingEnvs = $state.raw<{ cid: string; back: boolean } | null>(null);
	let error = $state<string | null>(null);
	let needToken = $state(false);
	let tokenInput = $state('');
	let editor = $state<ReturnType<typeof RequestEditor>>();
	let editorDirty = $state(false);
	let copied = $state(false);
	let refreshing = $state(false);
	let setting = $state.raw<{ cid: string; name: string; anchor: DOMRect } | null>(null);
	let unsetAnchor = $state<HTMLElement>();

	let collections = $derived(info?.collections ?? []);
	let byId = $derived(Object.fromEntries(collections.map((c) => [c.id, c])) as Record<string, CollectionInfo>);
	let active = $derived(
		(selected && byId[selected.cid]) || collections.find((c) => !c.missing) || null
	);
	let someCollapsed = $derived(anyCollapsed(collections.map((c) => c.id)));
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
	let runInsecure = $derived(
		runState !== null &&
			runState.collection === selected?.cid &&
			Object.values(runState.results).some((r) => r.insecure)
	);
	let copyTargets = $derived(collections.filter((c) => !c.missing && c.id !== docRef?.cid));
	let statusFns = $derived(Object.fromEntries(Object.entries(envVars).map(([cid, v]) => [cid, statusFrom(v)])));
	let infoFns = $derived(Object.fromEntries(Object.entries(envVars).map(([cid, v]) => [cid, infoFrom(v)])));
	let optionLists = $derived(Object.fromEntries(Object.entries(envVars).map(([cid, v]) => [cid, varOptions(v)])));
	/** Collection the command bar is about. */
	let barCid = $derived(docRef?.cid ?? (active && !active.missing ? active.id : null));
	let unsetVars = $derived.by(() => {
		const refs = barCid ? envVars[barCid]?.refs : undefined;
		return refs ? Object.keys(refs).filter((n) => needsValue(refs[n])) : [];
	});
	let unsetItems = $derived<MenuItem[]>(
		unsetVars.map((name) => ({
			label: name,
			icon: 'alert',
			hint: envVars[barCid!]?.refs[name],
			onselect: () => unsetAnchor && openSetVar(barCid!, name, unsetAnchor.getBoundingClientRect())
		}))
	);
	let requestMenu = $derived<MenuItem[]>([
		{ label: 'Copy as curl', icon: 'terminal', onselect: copyCurl },
		{
			label: 'Copy to collection…',
			icon: 'copy',
			disabled: copyTargets.length === 0,
			hint: copyTargets.length ? undefined : 'no other collection',
			onselect: () => (copying = true)
		},
		{ label: 'Delete file', icon: 'trash', danger: true, onselect: remove }
	]);

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
			const tree = await api.tree(c.id);
			trees = { ...trees, [c.id]: tree };
			await refreshEnvs(c.id);
		} catch (e) {
			fail(e);
		}
	}

	/** Re-reads the environment list, keeping the selection while it still exists. */
	async function refreshEnvs(cid: string) {
		const e = await api.envs(cid);
		envLists = { ...envLists, [cid]: e.envs };
		const saved = localStorage.getItem(ENV_KEY(cid));
		const def = e.default && e.envs.includes(e.default) ? e.default : null;
		envs[cid] = saved !== null && e.envs.includes(saved) ? saved : (def ?? e.envs[0] ?? '');
		await loadVars(cid);
	}

	async function envsChanged(cid: string, change: EnvChange) {
		if (change?.type === 'renamed' && envs[cid] === change.from) localStorage.setItem(ENV_KEY(cid), change.to);
		if (change?.type === 'deleted' && envs[cid] === change.name) localStorage.removeItem(ENV_KEY(cid));
		try {
			info = await api.info();
			await refreshEnvs(cid);
		} catch (e) {
			fail(e);
		}
	}

	async function loadVars(cid: string) {
		let next: EnvVars = { vars: [], refs: {} };
		try {
			next = await api.envVars(cid, envs[cid] ?? '');
		} catch {
			// unknown env or missing folder: no completions
		}
		envVars = { ...envVars, [cid]: next };
	}

	function openSetVar(cid: string, name: string, anchor: DOMRect) {
		setting = { cid, name, anchor };
	}

	async function copyCurl() {
		await editor?.copyCurl();
		copied = true;
		setTimeout(() => (copied = false), 1200);
	}

	async function refreshTree(cid: string) {
		if (!byId[cid] || byId[cid].missing) return;
		try {
			const tree = await api.tree(cid);
			trees = { ...trees, [cid]: tree };
		} catch (e) {
			fail(e);
		}
	}

	async function refreshCollections() {
		if (refreshing) return;
		refreshing = true;
		try {
			await api.refreshCollections();
			info = await api.info();
			await Promise.all(
				(info?.collections ?? [])
					.filter((c) => !c.missing)
					.map(async (c) => {
						await refreshTree(c.id);
						await refreshEnvs(c.id);
					})
			);
		} catch (e) {
			fail(e);
		} finally {
			refreshing = false;
		}
	}

	function collapseFolders() {
		for (const c of collections) if (!c.missing) collapseAll(c.id, trees[c.id] ?? null);
	}

	function expandFolders() {
		for (const c of collections) expandAll(c.id);
	}

	async function open(cid: string, path: string) {
		selected = { cid, path };
		reveal(cid, path);
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
		await Promise.all([refreshTree(ref.cid), loadVars(ref.cid)]);
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
		if (running !== null || !byId[cid] || byId[cid].missing) return;
		error = null;
		const isFile = path.endsWith('.sh');
		runState = isFile
			? null
			: {
					collection: cid,
					target: path,
					paths: [],
					results: {},
					current: null,
					summary: null,
					skipped: [],
					blocked: false
				};
		running = { cid, path };
		try {
			await run(
				cid,
				path,
				envs[cid] ?? '',
				(ev) => {
					if (ev.type === 'start' && runState) runState.paths = ev.paths;
					else if (ev.type === 'running') {
						running = { cid, path: ev.path };
						if (runState) runState.current = ev.path;
					} else if (ev.type === 'result') {
						results[key({ cid, path: ev.result.path })] = ev.result;
						if (runState) runState.results[ev.result.path] = ev.result;
						if (isFile) selected = { cid, path: ev.result.path };
					} else if (ev.type === 'cancelled' && runState) {
						runState.skipped = ev.skipped;
					} else if (ev.type === 'blocked' && runState) {
						runState.skipped = ev.skipped;
						runState.blocked = true;
					} else if (ev.type === 'done' && runState) {
						runState.summary = ev.summary;
						runState.current = null;
					}
				},
				(id) => (runId = id)
			);
			await loadVars(cid);
		} catch (e) {
			fail(e);
		} finally {
			if (runState && !runState.summary) runState.current = null;
			running = null;
			runId = null;
			stopping = false;
		}
	}

	async function stopRun() {
		if (!runId || stopping) return;
		stopping = true;
		try {
			await api.cancelRun(runId);
		} catch (e) {
			stopping = false;
			fail(e);
		}
	}

	async function changeEnv(cid: string, value: string) {
		envs[cid] = value;
		localStorage.setItem(ENV_KEY(cid), value);
		await loadVars(cid);
	}

	async function clearCaptures(cid: string) {
		try {
			await api.clearCaptures(cid, envs[cid] ?? '');
			await loadVars(cid);
		} catch (e) {
			fail(e);
		}
	}

	function closeEnvImport() {
		if (importingEnvs?.back) managingEnvs = importingEnvs.cid;
		importingEnvs = null;
	}

	async function addFolder(path: string) {
		const added = await api.addCollection(path);
		adding = false;
		info = await api.info();
		for (const col of added) await loadCollection(col);
		const first = added[0];
		if (first) selected = { cid: first.id, path: '' };
	}

	async function imported(col: CollectionInfo) {
		importing = false;
		info = await api.info();
		await loadCollection(col);
		selected = { cid: col.id, path: '' };
		doc = null;
		docRef = null;
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
			envVars = drop(envVars);
			for (const k of Object.keys(results)) if (k.startsWith(`${cid}:`)) delete results[k];
			if (selected?.cid === cid) selected = null;
			if (docRef?.cid === cid) {
				doc = null;
				docRef = null;
			}
			if (runState?.collection === cid) runState = null;
			if (managingEnvs === cid) managingEnvs = null;
			if (importingEnvs?.cid === cid) importingEnvs = null;
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
		const dialogOpen =
			newIn || adding || importing || copying || managingEnvs !== null || importingEnvs || needToken || setting;
		if (e.key === 'Escape' && runId && !dialogOpen && !e.defaultPrevented) {
			e.preventDefault();
			stopRun();
			return;
		}
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
		const onChange = async (ev: { changed?: string | null; collections?: boolean } | string) => {
			if (typeof ev === 'object' && ev.collections) {
				const known = new Set(collections.map((c) => c.id));
				info = await api.info();
				await Promise.all(
					(info?.collections ?? [])
						.filter((c) => !c.missing && !known.has(c.id))
						.map((c) => loadCollection(c))
				);
				return;
			}
			const cid = typeof ev === 'object' ? ev.changed : null;
			if (cid) {
				refreshTree(cid);
				if (byId[cid] && !byId[cid].missing) refreshEnvs(cid).catch(() => {});
			} else for (const c of collections) refreshTree(c.id);
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
		{#if doc && docRef}
			{@const colName = byId[docRef.cid]?.name ?? docRef.cid}
			<span class="sep" aria-hidden="true"></span>
			<span class="crumb" title="{colName}: {doc.path}">
				<span class="col">{colName}</span>
				<span class="slash">/</span>
				<span class="mono file">{doc.path}</span>
				{#if editorDirty}<span class="dirty" title="Unsaved changes" aria-label="Unsaved changes"></span>{/if}
			</span>
		{/if}
		<span class="spacer"></span>
		{#if unsetVars.length}
			<span class="unset" bind:this={unsetAnchor}>
				<Menu
					title="Variables without a value in this environment"
					label="{unsetVars.length} unset"
					icon="alert"
					class="unset-menu"
					items={unsetItems}
				/>
			</span>
		{/if}
		{#if doc && docRef}
			{#if copied}<span class="copied"><Icon name="check" size={12} />Copied</span>{/if}
			<Menu title="More actions for this request" items={requestMenu} />
			<span class="sep" aria-hidden="true"></span>
		{/if}
		<Menu
			title="Add a collection"
			label="Add collection"
			icon="plus"
			class="add-menu"
			items={[
				{ label: 'Open folder…', icon: 'folder', onselect: () => (adding = true) },
				{ label: 'Import from Postman…', icon: 'upload', onselect: () => (importing = true) }
			]}
		/>
	</header>

	{#if error}
		<div class="error-bar" role="alert">
			<Icon name="alert" />
			<span class="error-text">{error}</span>
			<button class="icon-btn" title="Dismiss" aria-label="Dismiss" onclick={() => (error = null)}>
				<Icon name="x" />
			</button>
		</div>
	{/if}

	<main
		bind:clientWidth={mainW}
		style:--sidebar-w="{sidebarW}px"
		style:--editor-fr="{editorFrac}fr"
		style:--response-fr="{1 - editorFrac}fr"
	>
		<aside>
			<div class="side-bar">
				<span class="side-title">Collections</span>
				<button
					class="icon-btn"
					title="Refresh collections"
					aria-label="Refresh collections"
					disabled={refreshing}
					onclick={refreshCollections}
				>
					<Icon name="refresh" size={13} />
				</button>
				<button
					class="icon-btn"
					title={someCollapsed ? 'Expand all' : 'Collapse all folders'}
					aria-label={someCollapsed ? 'Expand all' : 'Collapse all folders'}
					onclick={someCollapsed ? expandFolders : collapseFolders}
				>
					<Icon name={someCollapsed ? 'expand' : 'collapse'} size={13} />
				</button>
			</div>
			<div class="side-list">
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
						onmanageenvs={() => (managingEnvs = col.id)}
						onimportenvs={() => (importingEnvs = { cid: col.id, back: false })}
						onclearcaptures={() => clearCaptures(col.id)}
						onunlink={() => unlink(col.id)}
						onreveal={info?.can_reveal ? () => api.reveal(col.id).catch(fail) : undefined}
					/>
				{/each}
				{#if info && onlyScratch}
					<div class="onboard">
						<p>Try requests in Scratch, or bring in a collection:</p>
						<button class="ghost" onclick={() => (adding = true)}><Icon name="folder" />Open a folder</button>
						<button class="ghost" onclick={() => (importing = true)}><Icon name="upload" />Import from Postman</button>
					</div>
				{/if}
				{#if info && !info.saved}
					<p class="hint">Session only: changes to this list are not saved.</p>
				{/if}
			</div>
		</aside>
		<Splitter
			class="split-sidebar"
			label="Resize file explorer"
			value={sidebarW}
			min={SIDEBAR_MIN}
			max={SIDEBAR_MAX}
			onchange={(v) => (sidebarW = v)}
			onreset={() => (sidebarW = SIDEBAR_DEFAULT)}
		/>

		<section class="editor-col">
			{#if doc && docRef}
				{@const cid = docRef.cid}
				{#key `${cid}:${doc.path}:${docVersion}`}
					<RequestEditor
						bind:this={editor}
						bind:dirty={editorDirty}
						{cid}
						{doc}
						vars={optionLists[cid] ?? []}
						varStatus={statusFns[cid] ?? (() => 'ok')}
						varInfo={infoFns[cid] ?? infoFrom(undefined)}
						env={envs[cid] ?? ''}
						onsetvar={(name, anchor) => openSetVar(cid, name, anchor)}
						onsave={save}
						onrun={() => runPath(cid, doc!.path)}
						running={running !== null}
						{stopping}
						onstop={runId ? stopRun : undefined}
					/>
				{/key}
			{:else}
				<div class="placeholder">
					<img src="/logo.svg" alt="" width="56" height="56" />
					<p>Pick a request on the left, or create one with <Icon name="plus" size={12} class="inline" />.</p>
					<p class="shortcuts muted">
						<span><kbd>Ctrl</kbd> <kbd>S</kbd> save</span>
						<span><kbd>Ctrl</kbd> <kbd>Enter</kbd> run</span>
					</p>
				</div>
			{/if}
		</section>
		<Splitter
			class="split-editor"
			label="Resize editor"
			value={editorFrac * restW}
			min={COL_MIN}
			max={restW - COL_MIN}
			onchange={(v) => restW > 0 && (editorFrac = Math.min(0.95, Math.max(0.05, v / restW)))}
			onreset={() => (editorFrac = 0.5)}
		/>

		<section class="response-col">
			{#if runState}
				<RunPanel
					run={runState}
					name={byId[runState.collection]?.name ?? runState.collection}
					selected={selected?.cid === runState.collection ? selected.path : null}
					onpick={(p) => open(runState!.collection, p)}
					{stopping}
					onstop={runId && !runState.summary ? stopRun : undefined}
				/>
			{/if}
			<ResponsePane
				result={shownResult}
				{runInsecure}
				onadd={doc ? (k, l) => editor?.addAnnotation(k, l) : undefined} />
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

{#if importing}
	<ImportPostmanDialog saved={info?.saved ?? true} onimported={imported} onclose={() => (importing = false)} />
{/if}

{#if managingEnvs !== null}
	{@const cid = managingEnvs}
	<EnvManager
		{cid}
		collection={byId[cid]?.name ?? cid}
		current={envs[cid] ?? ''}
		onchanged={(change) => envsChanged(cid, change)}
		onimport={() => ((managingEnvs = null), (importingEnvs = { cid, back: true }))}
		onclose={() => (managingEnvs = null)}
	/>
{/if}

{#if importingEnvs}
	{@const cid = importingEnvs.cid}
	<ImportPostmanEnvDialog
		{cid}
		collection={byId[cid]?.name ?? cid}
		onimported={async () => {
			await envsChanged(cid, null);
			closeEnvImport();
		}}
		onclose={closeEnvImport}
	/>
{/if}

{#if setting}
	{@const s = setting}
	<SetVarPopover
		cid={s.cid}
		env={envs[s.cid] ?? ''}
		info={(infoFns[s.cid] ?? infoFrom(undefined))(s.name)}
		anchor={s.anchor}
		onsaved={() => loadVars(s.cid)}
		onmanage={() => ((managingEnvs = s.cid), (setting = null))}
		onclose={() => (setting = null)}
	/>
{/if}

{#if copying && docRef}
	<CopyDialog path={docRef.path} targets={copyTargets} oncopy={copyTo} onclose={() => (copying = false)} />
{/if}

{#if needToken}
	<div class="backdrop">
		<form class="dialog token" onsubmit={submitToken}>
			<h3>Token required</h3>
			<p class="muted">This server was started with <code>--token</code>.</p>
			<SecretInput bind:value={tokenInput} placeholder="token" aria-label="Token" />
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
		height: 44px;
		padding: 0 14px;
		border-bottom: 1px solid var(--border);
		background: var(--panel);
	}
	.brand {
		font-weight: 700;
		font-size: 15px;
		letter-spacing: 0.02em;
	}
	header :global(.add-menu) {
		color: var(--text);
		border-color: var(--border-strong);
	}
	header :global(button) {
		flex: none;
		white-space: nowrap;
	}
	.sep {
		flex: none;
		width: 1px;
		height: 20px;
		background: var(--border-strong);
		margin: 0 4px;
	}
	.crumb {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
		white-space: nowrap;
	}
	.crumb .col {
		color: var(--muted);
		flex: none;
		max-width: 30ch;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.crumb .slash {
		color: var(--border-strong);
	}
	.crumb .file {
		overflow: hidden;
		text-overflow: ellipsis;
		color: var(--text);
	}
	.dirty {
		flex: none;
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--warn);
	}
	.unset {
		display: flex;
		flex: none;
	}
	header :global(.unset-menu) {
		color: var(--warn);
		background: var(--warn-bg);
		border-color: var(--warn-border);
		min-height: 24px;
		font-size: 12px;
		border-radius: 999px;
	}
	header :global(.unset-menu:hover:not(:disabled)) {
		color: var(--warn);
		background: var(--warn-bg);
		border-color: var(--warn);
	}
	.copied {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--ok);
		font-size: 12px;
	}
	.spacer {
		flex: 1;
	}
	main {
		flex: 1;
		display: grid;
		grid-template-columns: var(--sidebar-w) 4px minmax(0, var(--editor-fr)) 4px minmax(0, var(--response-fr));
		min-height: 0;
	}
	aside {
		display: flex;
		flex-direction: column;
		min-height: 0;
		background: var(--panel);
	}
	.side-bar {
		display: flex;
		align-items: center;
		gap: 1px;
		padding: 4px 6px 4px 10px;
		border-bottom: 1px solid var(--border);
	}
	.side-title {
		flex: 1;
		font-size: 11px;
		font-weight: 600;
		color: var(--muted);
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}
	.side-list {
		flex: 1;
		min-height: 0;
		overflow: auto;
		padding: 0 0 6px;
	}
	.onboard {
		margin: 10px;
		padding: 12px;
		border: 1px dashed var(--border-strong);
		border-radius: var(--radius);
		display: flex;
		flex-direction: column;
		align-items: stretch;
		gap: 4px;
	}
	.onboard p {
		margin: 0 0 4px;
		color: var(--muted);
		font-size: 12px;
	}
	.onboard button {
		justify-content: flex-start;
		color: var(--text);
	}
	.hint {
		margin: 6px 12px;
		color: var(--muted);
		font-size: 12px;
	}
	.editor-col {
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
		gap: 6px;
		padding: 20px;
		text-align: center;
	}
	.placeholder img {
		opacity: 0.8;
		margin-bottom: 6px;
	}
	.placeholder p {
		margin: 0;
	}
	.placeholder :global(.inline) {
		display: inline-block;
		vertical-align: -1px;
	}
	.placeholder .shortcuts {
		display: flex;
		gap: 16px;
		margin-top: 8px;
		font-size: 12px;
	}
	.muted {
		color: var(--muted);
	}
	.error-bar {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 6px 10px 6px 14px;
		background: var(--fail-bg);
		border-bottom: 1px solid #5c2629;
		color: #ffd6d6;
	}
	.error-text {
		flex: 1;
		white-space: pre-wrap;
	}
	.error-bar button {
		color: inherit;
	}
	.token {
		width: 340px;
	}
	@media (max-width: 1100px) {
		main {
			grid-template-columns: var(--sidebar-w) 4px minmax(0, 1fr);
			grid-template-rows: minmax(0, 1fr) minmax(0, 1fr);
		}
		aside,
		main :global(.split-sidebar) {
			grid-row: 1 / span 2;
		}
		main :global(.split-editor) {
			display: none;
		}
		.editor-col {
			grid-column: 3;
			grid-row: 1;
			border-bottom: 1px solid var(--border);
		}
		.response-col {
			grid-column: 3;
			grid-row: 2;
		}
	}
</style>
