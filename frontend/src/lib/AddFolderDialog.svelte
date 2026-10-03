<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from './api';
	import type { DirListing } from './types';

	interface Props {
		saved: boolean;
		onadd: (path: string) => Promise<void>;
		onclose: () => void;
	}

	let { saved, onadd, onclose }: Props = $props();

	interface NativeDialog {
		open(options: { directory: boolean; title?: string; defaultPath?: string }): Promise<string | string[] | null>;
	}

	/** The desktop app's native folder picker; absent in a browser. */
	const nativeDialog = (globalThis as { __TAURI__?: { dialog?: NativeDialog } }).__TAURI__?.dialog;

	let path = $state('');
	let listing = $state.raw<DirListing | null>(null);
	let error = $state<string | null>(null);
	let busy = $state(false);

	async function browse(to?: string) {
		error = null;
		try {
			listing = await api.listDirs(to);
			path = listing.path;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function add(folder: string) {
		error = null;
		busy = true;
		try {
			await onadd(folder);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (path.trim()) await add(path.trim());
	}

	async function choose() {
		if (!nativeDialog) return;
		error = null;
		try {
			const picked = await nativeDialog.open({
				directory: true,
				title: 'Choose a collection folder',
				defaultPath: path.trim() || undefined
			});
			if (typeof picked === 'string') {
				path = picked;
				await add(picked);
			}
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	onMount(() => {
		browse();
	});
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
	<form class="dialog" onsubmit={submit} aria-label="Add folder">
		<h3>Add a collection folder</h3>
		<p class="muted">
			Each folder keeps its own environments, captured values and trust.
			{#if !saved}This server was started with folder arguments, so the folder is added for this session only.{/if}
		</p>
		<div class="row">
			<input class="mono" bind:value={path} placeholder="/path/to/collection" aria-label="Folder path" />
			<button type="button" onclick={() => browse(path)}>Go</button>
			{#if nativeDialog}
				<button type="button" onclick={choose} disabled={busy}>Choose...</button>
			{/if}
		</div>
		{#if listing}
			<ul class="dirs">
				{#if listing.parent}
					<li><button type="button" onclick={() => browse(listing!.parent!)}>..</button></li>
				{/if}
				{#each listing.dirs as d (d.path)}
					<li>
						<button type="button" onclick={() => browse(d.path)} ondblclick={() => add(d.path)}>
							<span>{d.name}/</span>
							{#if d.collection}<span class="tag">collection</span>{/if}
						</button>
					</li>
				{:else}
					<li class="muted none">No subfolders.</li>
				{/each}
			</ul>
		{/if}
		{#if listing?.collection}
			<p class="ok">This folder looks like a Sankh collection.</p>
		{/if}
		{#if error}<p class="error">{error}</p>{/if}
		<div class="buttons">
			<button type="button" onclick={onclose}>Cancel</button>
			<button type="submit" class="primary" disabled={busy || !path.trim()}>Add folder</button>
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
		width: min(560px, 92vw);
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
	.row {
		display: flex;
		gap: 6px;
	}
	.row input {
		flex: 1;
	}
	.dirs {
		list-style: none;
		margin: 0;
		padding: 4px;
		height: 260px;
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
	.dirs button:hover {
		background: var(--panel-2);
	}
	.tag {
		font-size: 10px;
		color: var(--ok);
	}
	.none {
		padding: 3px 8px;
	}
	.muted {
		color: var(--muted);
	}
	.ok {
		color: var(--ok);
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
