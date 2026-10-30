<script lang="ts">
	import { onMount } from 'svelte';
	import Icon from './Icon.svelte';
	import { ApiError, api } from './api';
	import { formatSize } from './format';
	import type { FileListing } from './types';

	interface Props {
		cid: string;
		/** Path currently in the field; its folder opens first. */
		current?: string;
		onpick: (path: string) => void;
		onclose: () => void;
	}

	let { cid, current = '', onpick, onclose }: Props = $props();

	const UPLOAD_DIR = 'files';

	let listing = $state.raw<FileListing | null>(null);
	let error = $state<string | null>(null);
	let busy = $state(false);
	let dragging = $state(false);
	let target = $state(UPLOAD_DIR);
	/** An upload that clashed with an existing file. */
	let conflict = $state.raw<{ file: File; path: string } | null>(null);

	let crumbs = $derived(
		listing?.dir ? listing.dir.split('/').map((name, i, all) => ({ name, path: all.slice(0, i + 1).join('/') })) : []
	);

	async function browse(dir: string) {
		error = null;
		try {
			listing = await api.listFiles(cid, dir);
		} catch (e) {
			if (dir && e instanceof ApiError && e.status === 404) return browse('');
			error = e instanceof Error ? e.message : String(e);
		}
	}

	function folderOf(path: string) {
		const i = path.lastIndexOf('/');
		return i === -1 ? '' : path.slice(0, i);
	}

	function joinPath(dir: string, name: string) {
		const d = dir.trim().replace(/^\/+|\/+$/g, '');
		return d ? `${d}/${name}` : name;
	}

	/** `logo.png` to `logo-1.png`. */
	function numbered(path: string, n: number) {
		const slash = path.lastIndexOf('/');
		const dot = path.lastIndexOf('.');
		return dot > slash + 1 ? `${path.slice(0, dot)}-${n}${path.slice(dot)}` : `${path}-${n}`;
	}

	async function upload(file: File, copy = false) {
		error = null;
		conflict = null;
		busy = true;
		const base = joinPath(target, file.name);
		try {
			for (let n = copy ? 1 : 0; n < 100; n++) {
				const path = n ? numbered(base, n) : base;
				try {
					const saved = await api.uploadFile(cid, path, file);
					onpick(saved.path);
					return;
				} catch (e) {
					if (!(e instanceof ApiError && e.status === 409)) throw e;
					if (!copy) {
						conflict = { file, path };
						return;
					}
				}
			}
			error = `Too many copies of ${base}.`;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}

	function chosen(e: Event & { currentTarget: HTMLInputElement }) {
		const file = e.currentTarget.files?.[0];
		e.currentTarget.value = '';
		if (file) upload(file);
	}

	function dropped(e: DragEvent) {
		e.preventDefault();
		dragging = false;
		const file = e.dataTransfer?.files[0];
		if (file) upload(file);
	}

	onMount(() => {
		browse(folderOf(current.trim()));
	});
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
	<div class="dialog" role="dialog" aria-label="Choose a file">
		<h3>Choose a file</h3>
		<p class="muted">Requests send files by a path relative to the collection folder, so the file must be inside it.</p>

		<section
			class={['drop', dragging && 'over']}
			aria-label="Upload a file"
			ondragover={(e) => {
				e.preventDefault();
				dragging = true;
			}}
			ondragleave={() => (dragging = false)}
			ondrop={dropped}
		>
			<Icon name="upload" size={16} />
			<span>Drop a file here or</span>
			<label class="pick">
				browse your computer
				<input type="file" onchange={chosen} disabled={busy} />
			</label>
			<span class="into">
				and copy it into
				<input class="mono" bind:value={target} aria-label="Upload folder" placeholder="collection root" />
			</span>
		</section>
		{#if conflict}
			<div class="conflict">
				<span><code>{conflict.path}</code> already exists.</span>
				<button type="button" onclick={() => onpick(conflict!.path)}>Use the existing file</button>
				<button type="button" onclick={() => upload(conflict!.file, true)}>Save a copy</button>
			</div>
		{/if}

		<div class="crumbs mono">
			<button type="button" onclick={() => browse('')}>collection</button>
			{#each crumbs as c (c.path)}
				<span class="muted">/</span><button type="button" onclick={() => browse(c.path)}>{c.name}</button>
			{/each}
		</div>
		{#if listing}
			<ul class="files">
				{#if listing.parent !== null}
					<li><button type="button" onclick={() => browse(listing!.parent ?? '')}>..</button></li>
				{/if}
				{#each listing.dirs as d (d.path)}
					<li>
						<button type="button" onclick={() => browse(d.path)}>
							<span><Icon name="folder" size={12} />{d.name}/</span>
						</button>
					</li>
				{/each}
				{#each listing.files as f (f.path)}
					<li>
						<button type="button" class={{ current: f.path === current }} onclick={() => onpick(f.path)}>
							<span class="mono">{f.name}</span>
							<span class="muted">{formatSize(f.size)}</span>
						</button>
					</li>
				{/each}
				{#if !listing.dirs.length && !listing.files.length}
					<li class="muted none">Empty folder.</li>
				{/if}
			</ul>
		{/if}
		{#if error}<p class="error">{error}</p>{/if}
		<div class="buttons">
			<button type="button" onclick={onclose}>Cancel</button>
		</div>
	</div>
</div>

<style>
	p {
		margin: 0;
	}
	.muted {
		color: var(--muted);
	}
	.error {
		color: var(--fail);
	}
	.drop {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		padding: 14px;
		border: 1px dashed var(--border-strong);
		border-radius: 6px;
		color: var(--muted);
	}
	.drop.over {
		border-color: var(--accent);
		background: var(--panel-2);
	}
	.pick {
		color: var(--accent);
		cursor: pointer;
		text-decoration: underline;
	}
	.pick input {
		display: none;
	}
	.into {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	.into input {
		width: 140px;
		min-height: 24px;
		font-size: 12px;
	}
	.conflict {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
		padding: 8px 10px;
		border: 1px solid var(--warn-border);
		background: var(--warn-bg);
		border-radius: 6px;
	}
	.conflict span {
		flex: 1;
	}
	.crumbs {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 2px;
		font-size: 12px;
	}
	.crumbs button {
		border: none;
		background: none;
		padding: 0 4px;
		min-height: 20px;
	}
	.crumbs button:hover {
		color: var(--accent);
	}
	.files {
		list-style: none;
		margin: 0;
		padding: 4px;
		height: 240px;
		overflow: auto;
		border: 1px solid var(--border);
		border-radius: 6px;
		background: var(--bg);
	}
	.files button {
		width: 100%;
		display: flex;
		justify-content: space-between;
		gap: 10px;
		background: none;
		border: none;
		text-align: left;
		padding: 3px 8px;
	}
	.files button span {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	.files button:hover,
	.files button.current {
		background: var(--panel-2);
	}
	.none {
		padding: 3px 8px;
	}
</style>
