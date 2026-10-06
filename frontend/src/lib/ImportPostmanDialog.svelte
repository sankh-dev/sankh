<script lang="ts">
	import { ApiError, api } from './api';
	import type { CollectionInfo, DirListing, PostmanPreview } from './types';

	interface Props {
		saved: boolean;
		onimported: (collection: CollectionInfo) => Promise<void>;
		onclose: () => void;
	}

	let { saved, onimported, onclose }: Props = $props();

	interface NativeDialog {
		open(options: { directory: boolean; title?: string; defaultPath?: string }): Promise<string | string[] | null>;
	}

	/** The desktop app's native folder picker; absent in a browser. */
	const nativeDialog = (globalThis as { __TAURI__?: { dialog?: NativeDialog } }).__TAURI__?.dialog;

	let collectionText = $state<string | null>(null);
	let collectionFile = $state('');
	let envTexts = $state.raw<string[]>([]);
	let envFiles = $state.raw<string[]>([]);
	let preview = $state.raw<PostmanPreview | null>(null);
	let parent = $state('');
	let name = $state('');
	let listing = $state.raw<DirListing | null>(null);
	let force = $state(false);
	let conflict = $state(false);
	let error = $state<string | null>(null);
	let busy = $state(false);

	let warningsByPath = $derived.by(() => {
		const groups: Record<string, string[]> = {};
		for (const w of preview?.report.warnings ?? []) (groups[w.path || 'collection'] ??= []).push(w.message);
		return Object.entries(groups);
	});
	let showOverwrite = $derived(conflict || (preview?.nonempty ?? false));
	let dir = $derived(parent.trim() && name.trim() ? join(parent.trim(), name.trim()) : '');

	function message(e: unknown) {
		return e instanceof Error ? e.message : String(e);
	}

	function split(path: string): [string, string] {
		const trimmed = path.replace(/[\\/]+$/, '');
		const cut = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
		return cut < 0 ? ['', trimmed] : [trimmed.slice(0, cut) || trimmed.slice(0, 1), trimmed.slice(cut + 1)];
	}

	function join(base: string, child: string) {
		const sep = base.includes('\\') && !base.includes('/') ? '\\' : '/';
		return base.endsWith(sep) ? base + child : base + sep + child;
	}

	/** Without a destination the server picks a default, which is only wanted for a newly picked file. */
	async function refresh(useDir = dir) {
		if (collectionText === null || (!useDir && preview)) return;
		error = null;
		try {
			preview = await api.previewPostman({
				collection: collectionText,
				envs: envTexts,
				dir: useDir || undefined
			});
			[parent, name] = split(preview.dir);
			conflict = false;
			force = false;
		} catch (e) {
			preview = null;
			error = message(e);
		}
	}

	async function pickCollection(event: Event) {
		const file = (event.currentTarget as HTMLInputElement).files?.[0];
		if (!file) return;
		collectionFile = file.name;
		collectionText = await file.text();
		preview = null;
		await refresh('');
	}

	/** Lists `to`, or its nearest existing ancestor (the default parent may not exist yet). */
	async function browse(to: string) {
		error = null;
		let target = to.trim();
		for (;;) {
			try {
				listing = await api.listDirs(target || undefined);
				break;
			} catch (e) {
				const [up] = split(target);
				if (!target || up === target) {
					error = message(e);
					return;
				}
				target = up;
			}
		}
		if (target !== to.trim()) return;
		parent = listing.path;
		await refresh();
	}

	async function pickEnvs(event: Event) {
		const files = Array.from((event.currentTarget as HTMLInputElement).files ?? []);
		envFiles = files.map((f) => f.name);
		envTexts = await Promise.all(files.map((f) => f.text()));
		await refresh();
	}

	async function choose() {
		if (!nativeDialog) return;
		error = null;
		try {
			const picked = await nativeDialog.open({
				directory: true,
				title: 'Choose the folder to create the collection in',
				defaultPath: parent.trim() || undefined
			});
			if (typeof picked === 'string') {
				parent = picked;
				await refresh();
			}
		} catch (e) {
			error = message(e);
		}
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (collectionText === null || !dir) return;
		error = null;
		busy = true;
		try {
			const done = await api.importPostman({
				collection: collectionText,
				envs: envTexts,
				dir,
				force
			});
			await onimported(done.collection);
		} catch (e) {
			if (e instanceof ApiError && e.status === 409 && e.message.includes('not empty')) conflict = true;
			error = message(e);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
	<form class="dialog" onsubmit={submit} aria-label="Import from Postman">
		<h3>Import from Postman</h3>
		<p class="muted">
			Converts a Postman Collection v2.0 / v2.1 export into a new collection folder. Nothing is written until you
			choose <strong>Import</strong>.
			{#if !saved}This server was started with folder arguments, so the new folder is added for this session only.{/if}
		</p>

		<label class="field">
			<span>Collection export</span>
			<input type="file" accept=".json,application/json" onchange={pickCollection} />
			{#if collectionFile}<span class="muted mono">{collectionFile}</span>{/if}
		</label>
		<label class="field">
			<span>Environments (optional)</span>
			<input type="file" accept=".json,application/json" multiple onchange={pickEnvs} />
			{#if envFiles.length}<span class="muted mono">{envFiles.join(', ')}</span>{/if}
		</label>

		{#if preview}
			{@const r = preview.report}
			<fieldset class="dest">
				<legend>Where to create it</legend>
				<label class="field">
					<span>Save in</span>
					<div class="row">
						<input
							class="mono"
							bind:value={parent}
							onchange={() => refresh()}
							placeholder="/path/to/parent"
							aria-label="Parent folder"
						/>
						{#if nativeDialog}
							<button type="button" onclick={choose} disabled={busy}>Choose...</button>
						{:else}
							<button
								type="button"
								onclick={() => (listing ? (listing = null) : browse(parent))}
								disabled={busy}
								aria-expanded={listing !== null}
							>
								{listing ? 'Done' : 'Browse...'}
							</button>
						{/if}
					</div>
				</label>
				{#if listing}
					<ul class="dirs" aria-label="Folders in {listing.path}">
						{#if listing.parent}
							<li><button type="button" onclick={() => browse(listing!.parent!)}>..</button></li>
						{/if}
						{#each listing.dirs as d (d.path)}
							<li>
								<button type="button" onclick={() => browse(d.path)} disabled={d.collection}>
									<span>{d.name}/</span>
									{#if d.collection}<span class="tag">collection</span>{/if}
								</button>
							</li>
						{:else}
							<li class="muted none">No subfolders.</li>
						{/each}
					</ul>
				{/if}
				<label class="field">
					<span>Folder name</span>
					<input class="mono" bind:value={name} onchange={() => refresh()} aria-label="New folder name" />
				</label>
				{#if dir}<p class="muted">Creates <span class="mono">{dir}</span></p>{/if}
				{#if showOverwrite}
					<label class="check warn">
						<input type="checkbox" bind:checked={force} />
						This folder is not empty. Overwrite files with the same name.
					</label>
				{/if}
			</fieldset>

			<div class="summary">
				<p>
					<strong>{r.collection}</strong>: {r.requests} request(s) in {r.folders} folder(s), {preview.files.length}
					file(s){#if r.environments.length}, environments: {r.environments.join(', ')}{/if}
				</p>
				{#if r.renamed.length}
					<details>
						<summary>{r.renamed.length} renamed variable(s)</summary>
						<ul class="mono">
							{#each r.renamed as x (x.from)}<li>{x.from} &rarr; {x.to}</li>{/each}
						</ul>
					</details>
				{/if}
				{#if r.placeholders.length}
					<details open>
						<summary>Set these in <code>.env.local</code> (see <code>.env.example</code>)</summary>
						<p class="muted">
							<code>.env.local</code> is created with sample values guessed from each name (an existing one is kept).
							Replace them before real use.
						</p>
						<ul>
							{#each r.placeholders as p (p.name)}
								<li><span class="mono">{p.name}</span> <span class="muted">{p.note}</span></li>
							{/each}
						</ul>
					</details>
				{/if}
				{#if warningsByPath.length}
					<details>
						<summary class="warn">{r.warnings.length} warning(s)</summary>
						{#each warningsByPath as [path, messages] (path)}
							<p class="mono path">{path}</p>
							<ul>
								{#each messages as m, i (i)}<li>{m}</li>{/each}
							</ul>
						{/each}
					</details>
				{/if}
			</div>
		{/if}

		{#if error}<p class="error">{error}</p>{/if}
		<div class="buttons">
			<button type="button" onclick={onclose}>Cancel</button>
			<button
				type="submit"
				class="primary"
				disabled={busy || !preview || !dir || (showOverwrite && !force)}
			>
				{busy ? 'Importing...' : name.trim() ? `Import to ${name.trim()}/` : 'Import'}
			</button>
		</div>
	</form>
</div>

<style>
	.dialog {
		width: min(620px, 92vw);
	}
	p {
		margin: 0;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.row {
		display: flex;
		gap: 6px;
	}
	.row input {
		flex: 1;
	}
	.dest {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin: 0;
		padding: 8px 10px 10px;
		border: 1px solid var(--border);
		border-radius: 6px;
	}
	.dest legend {
		padding: 0 4px;
		font-weight: 600;
	}
	.dirs {
		list-style: none;
		margin: 0;
		padding: 4px;
		height: 180px;
		overflow: auto;
		border: 1px solid var(--border);
		border-radius: 6px;
		background: var(--bg);
	}
	.dirs button {
		width: 100%;
		display: flex;
		justify-content: space-between;
		background: none;
		border: none;
		text-align: left;
		padding: 3px 8px;
	}
	.dirs button:hover:not(:disabled) {
		background: var(--panel-2);
	}
	.tag {
		font-size: 10px;
		color: var(--ok);
	}
	.none {
		padding: 3px 8px;
	}
	.summary {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-radius: 6px;
		background: var(--bg);
		max-height: 300px;
		overflow: auto;
	}
	ul {
		margin: 4px 0;
		padding-left: 20px;
	}
	.path {
		margin-top: 6px;
	}
	.check {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.muted {
		color: var(--muted);
	}
	.warn {
		color: var(--warn);
	}
	.error {
		color: var(--fail);
	}
</style>
