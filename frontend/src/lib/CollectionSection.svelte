<script lang="ts">
	import { untrack } from 'svelte';
	import Tree from './Tree.svelte';
	import type { CollectionInfo, TreeNode } from './types';

	interface Props {
		col: CollectionInfo;
		tree: TreeNode | null;
		envs: string[];
		env: string;
		/** Selected / running request path in this collection, if any. */
		selected: string | null;
		running: string | null;
		outcomes: Record<string, string>;
		busy: boolean;
		onselect: (path: string) => void;
		onrun: (path: string) => void;
		onnew: (folder: string) => void;
		onenv: (env: string) => void;
		onunlink: () => void;
	}

	let {
		col,
		tree,
		envs,
		env,
		selected,
		running,
		outcomes,
		busy,
		onselect,
		onrun,
		onnew,
		onenv,
		onunlink
	}: Props = $props();

	const COLLAPSED_KEY = (id: string) => `sankh-collapsed:${id}`;
	// Sections are keyed by collection id, so the id never changes here.
	let open = $state(untrack(() => localStorage.getItem(COLLAPSED_KEY(col.id)) === null));
	let trusted = $derived(col.trust?.state === 'trusted');
	let children = $derived(tree?.type === 'folder' ? tree.children : []);

	function toggle() {
		open = !open;
		if (open) localStorage.removeItem(COLLAPSED_KEY(col.id));
		else localStorage.setItem(COLLAPSED_KEY(col.id), '1');
	}
</script>

<section class={['collection', col.missing && 'missing']}>
	<div class="head">
		<button class="toggle" onclick={toggle} aria-expanded={open} title={col.error ?? col.root}>
			<span class="chev">{open ? '▾' : '▸'}</span>
			<span class="name">{col.name}</span>
			{#if col.missing}
				<span class="badge bad">missing</span>
			{:else if col.trust?.state === 'changed'}
				<span class="badge warn" title="Changed since it was trusted">changed</span>
			{:else if !trusted}
				<span class="badge warn" title="Not trusted: nothing runs yet">untrusted</span>
			{/if}
		</button>
		<span class="actions">
			{#if !col.missing}
				<button class="icon" title="New request" onclick={() => onnew('')}>+</button>
				<button class="icon" title="Run collection" disabled={!trusted || busy} onclick={() => onrun('')}>▶</button>
			{/if}
			{#if !col.scratch}
				<button class="icon" title="Unlink from workspace (files stay on disk)" onclick={onunlink}>✕</button>
			{/if}
		</span>
	</div>
	{#if open && !col.missing}
		<div class="env">
			<select
				value={env}
				onchange={(e) => onenv(e.currentTarget.value)}
				aria-label="Environment for {col.name}"
				title="Environment for {col.name}"
			>
				<option value="">no env</option>
				{#each envs as e (e)}
					<option value={e}>{e}</option>
				{/each}
			</select>
		</div>
		{#each children as child (child.path)}
			<Tree node={child} {selected} {running} {outcomes} depth={1} {onselect} {onrun} {onnew} />
		{:else}
			<p class="empty">No requests yet.</p>
		{/each}
	{:else if open && col.missing}
		<p class="empty">{col.error ?? 'Folder not found.'}</p>
	{/if}
</section>

<style>
	.collection {
		padding-bottom: 6px;
		border-bottom: 1px solid var(--border);
	}
	.collection.missing .name {
		color: var(--muted);
		text-decoration: line-through;
	}
	.head {
		display: flex;
		align-items: center;
		padding: 4px 4px 2px 6px;
		position: sticky;
		top: 0;
		background: var(--panel);
		z-index: 1;
	}
	.toggle {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 6px;
		background: none;
		border: none;
		padding: 4px 2px;
		text-align: left;
		min-width: 0;
	}
	.chev {
		width: 10px;
		color: var(--muted);
	}
	.name {
		font-weight: 700;
		text-transform: uppercase;
		font-size: 11px;
		letter-spacing: 0.05em;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.badge {
		font-size: 10px;
		padding: 0 5px;
		border-radius: 4px;
		border: 1px solid currentColor;
		flex: none;
	}
	.badge.warn {
		color: var(--warn);
	}
	.badge.bad {
		color: var(--fail);
	}
	.actions {
		display: flex;
		gap: 2px;
		opacity: 0.35;
	}
	.head:hover .actions,
	.head:focus-within .actions {
		opacity: 1;
	}
	.icon {
		padding: 0 6px;
		font-size: 11px;
		line-height: 18px;
		background: none;
	}
	.env {
		padding: 0 10px 4px 22px;
	}
	.env select {
		width: 100%;
		padding: 2px 6px;
		font-size: 12px;
	}
	.empty {
		margin: 2px 0 4px;
		padding-left: 24px;
		color: var(--muted);
		font-size: 12px;
	}
</style>
