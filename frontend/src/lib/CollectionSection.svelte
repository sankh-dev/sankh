<script lang="ts">
	import { untrack } from 'svelte';
	import Icon from './Icon.svelte';
	import Menu, { type MenuItem } from './Menu.svelte';
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
		onmanageenvs: () => void;
		onclearcaptures: () => void;
		onunlink: () => void;
		/** Opens the folder in the file manager; absent when the server is remote. */
		onreveal?: () => void;
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
		onmanageenvs,
		onclearcaptures,
		onunlink,
		onreveal
	}: Props = $props();

	const COLLAPSED_KEY = (id: string) => `sankh-collapsed:${id}`;
	// Sections are keyed by collection id, so the id never changes here.
	let open = $state(untrack(() => localStorage.getItem(COLLAPSED_KEY(col.id)) === null));
	let trusted = $derived(col.trust?.state === 'trusted');
	let children = $derived(tree?.type === 'folder' ? tree.children : []);
	let menu = $derived.by(() => {
		const items: MenuItem[] = [];
		if (!col.missing) {
			if (onreveal) items.push({ label: 'Open folder', icon: 'folder', onselect: onreveal });
			items.push(
				{ label: 'Manage environments…', icon: 'sliders', onselect: onmanageenvs },
				{ label: 'Clear captures', icon: 'eraser', hint: env || undefined, onselect: onclearcaptures }
			);
		}
		if (!col.scratch) {
			items.push({ label: 'Unlink from workspace', icon: 'unlink', danger: true, onselect: onunlink });
		}
		return items;
	});

	function toggle() {
		open = !open;
		if (open) localStorage.removeItem(COLLAPSED_KEY(col.id));
		else localStorage.setItem(COLLAPSED_KEY(col.id), '1');
	}
</script>

<section class={['collection', col.missing && 'missing']}>
	<div class="head">
		<button class="toggle" onclick={toggle} aria-expanded={open} title={col.error ?? col.root}>
			<Icon name="chevron" size={12} class={open ? 'chev open' : 'chev'} />
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
				<button class="icon-btn" title="New request" aria-label="New request in {col.name}" onclick={() => onnew('')}>
					<Icon name="plus" />
				</button>
				<button
					class="icon-btn run"
					title="Run collection"
					aria-label="Run {col.name}"
					disabled={!trusted || busy}
					onclick={() => onrun('')}
				>
					<Icon name="play" size={12} />
				</button>
			{/if}
			{#if menu.length}
				<Menu title="More actions for {col.name}" items={menu} />
			{/if}
		</span>
	</div>
	{#if open && !col.missing}
		<div class="env">
			<select
				value={env}
				onchange={(e) => onenv(e.currentTarget.value)}
				aria-label="Environment for {col.name}"
				title={env
					? `environments/${env}.env, with local overrides (.env.local) on top`
					: 'No environments yet: create one in Manage environments'}
				disabled={envs.length === 0}
			>
				{#each envs as e (e)}
					<option value={e}>{e}</option>
				{:else}
					<option value="">No environments</option>
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
		padding-bottom: 8px;
		border-bottom: 1px solid var(--border);
	}
	.collection.missing .name {
		color: var(--muted);
		text-decoration: line-through;
	}
	.head {
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 6px 6px 4px 6px;
		position: sticky;
		top: 0;
		background: var(--panel);
		z-index: 1;
	}
	.toggle {
		flex: 1;
		justify-content: flex-start;
		gap: 6px;
		background: none;
		border: none;
		padding: 0 4px;
		text-align: left;
		min-width: 0;
	}
	.toggle:hover:not(:disabled) {
		background: none;
	}
	.toggle :global(.chev) {
		color: var(--muted);
		transition: transform 0.12s;
	}
	.toggle :global(.chev.open) {
		transform: rotate(90deg);
	}
	.name {
		font-weight: 700;
		text-transform: uppercase;
		font-size: 11px;
		letter-spacing: 0.06em;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.badge {
		font-size: 10px;
		font-weight: 600;
		padding: 1px 6px;
		border-radius: 999px;
		flex: none;
	}
	.badge.warn {
		color: var(--warn);
		background: var(--warn-bg);
	}
	.badge.bad {
		color: var(--fail);
		background: var(--fail-bg);
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 1px;
		opacity: 0.5;
		transition: opacity 0.12s;
	}
	.head:hover .actions,
	.head:focus-within .actions {
		opacity: 1;
	}
	.run:hover:not(:disabled) {
		color: var(--ok);
	}
	.env {
		padding: 0 10px 6px 26px;
	}
	.env select {
		width: 100%;
		font-size: 12px;
	}
	.empty {
		margin: 2px 0 4px;
		padding-left: 28px;
		color: var(--muted);
		font-size: 12px;
	}
</style>
