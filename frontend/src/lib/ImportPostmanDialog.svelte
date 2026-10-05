<script lang="ts">
	import { ApiError, api } from './api';
	import type { CollectionInfo, PostmanPreview } from './types';

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
	let dir = $state('');
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

	function message(e: unknown) {
		return e instanceof Error ? e.message : String(e);
	}

	async function refresh(useDir = dir) {
		if (collectionText === null) return;
		error = null;
		try {
			preview = await api.previewPostman({
				collection: collectionText,
				envs: envTexts,
				dir: useDir.trim() || undefined
			});
			dir = preview.dir;
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
		await refresh('');
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
				title: 'Choose where to write the collection',
				defaultPath: dir.trim() || undefined
			});
			if (typeof picked === 'string') await refresh(picked);
		} catch (e) {
			error = message(e);
		}
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (collectionText === null || !dir.trim()) return;
		error = null;
		busy = true;
		try {
			const done = await api.importPostman({
				collection: collectionText,
				envs: envTexts,
				dir: dir.trim(),
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

			<label class="field">
				<span>Write to folder</span>
				<div class="row">
					<input
						class="mono"
						bind:value={dir}
						onchange={() => refresh()}
						placeholder="/path/to/new-collection"
						aria-label="Destination folder"
					/>
					{#if nativeDialog}
						<button type="button" onclick={choose} disabled={busy}>Choose...</button>
					{/if}
				</div>
			</label>
			{#if showOverwrite}
				<label class="check warn">
					<input type="checkbox" bind:checked={force} />
					This folder is not empty. Overwrite files with the same name.
				</label>
			{/if}
		{/if}

		{#if error}<p class="error">{error}</p>{/if}
		<div class="buttons">
			<button type="button" onclick={onclose}>Cancel</button>
			<button
				type="submit"
				class="primary"
				disabled={busy || !preview || !dir.trim() || (showOverwrite && !force)}
			>
				{busy ? 'Importing...' : 'Import'}
			</button>
		</div>
	</form>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: #0008;
		display: grid;
		place-items: center;
		z-index: 10;
	}
	.dialog {
		width: min(620px, 92vw);
		max-height: 90vh;
		overflow: auto;
		background: var(--panel);
		border: 1px solid var(--border);
		border-radius: 10px;
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	h3,
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
	.buttons {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}
</style>
