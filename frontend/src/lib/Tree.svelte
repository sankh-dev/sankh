<script lang="ts">
	import Icon from './Icon.svelte';
	import Tree from './Tree.svelte';
	import type { TreeNode } from './types';

	interface Props {
		node: TreeNode;
		selected: string | null;
		running: string | null;
		outcomes: Record<string, string>;
		depth?: number;
		onselect: (path: string) => void;
		onrun: (path: string) => void;
		onnew: (folder: string) => void;
	}

	let { node, selected, running, outcomes, depth = 0, onselect, onrun, onnew }: Props = $props();
	let open = $state(true);
</script>

{#if node.type === 'folder'}
	<div class="row folder" style:--depth={depth}>
		<button class="toggle" onclick={() => (open = !open)} aria-expanded={open}>
			<Icon name="chevron" size={12} class={open ? 'chev open' : 'chev'} />
			<span class="label">{node.name}</span>
		</button>
		<span class="actions">
			<button class="icon-btn" title="New request here" aria-label="New request in {node.name}" onclick={() => onnew(node.path)}>
				<Icon name="plus" size={13} />
			</button>
			<button class="icon-btn run" title="Run folder" aria-label="Run {node.name}" onclick={() => onrun(node.path)}>
				<Icon name="play" size={11} />
			</button>
		</span>
	</div>
	{#if open}
		{#each node.children as child (child.path)}
			<Tree node={child} {selected} {running} {outcomes} depth={depth + 1} {onselect} {onrun} {onnew} />
		{/each}
	{/if}
{:else}
	{@const method = node.method ?? (node.raw ? 'RAW' : '?')}
	<div class={['row', 'request', selected === node.path && 'selected']} style:--depth={depth}>
		<button class="toggle" onclick={() => onselect(node.path)} title={node.path}>
			<span class="method method-{method}">{method}</span>
			<span class="label">{node.name}</span>
			{#if node.errors > 0}
				<span class="err" title="{node.errors} annotation error(s)"><Icon name="alert" size={12} /></span>
			{/if}
			{#if running === node.path}
				<span class="dot running" title="running"></span>
			{:else if outcomes[node.path]}
				<span class="dot {outcomes[node.path]}" title={outcomes[node.path]}></span>
			{/if}
		</button>
		<span class="actions">
			<button class="icon-btn run" title="Run" aria-label="Run {node.name}" onclick={() => onrun(node.path)}>
				<Icon name="play" size={11} />
			</button>
		</span>
	</div>
{/if}

<style>
	.row {
		display: flex;
		align-items: center;
		padding-left: calc(var(--depth) * 12px + 4px);
		padding-right: 4px;
		border-radius: var(--radius-sm);
		margin: 1px 6px;
		transition: background-color 0.1s;
	}
	.row:hover {
		background: var(--panel-2);
	}
	.row.selected {
		background: var(--selected);
		box-shadow: inset 2px 0 0 var(--accent);
	}
	.toggle {
		flex: 1;
		justify-content: flex-start;
		gap: 6px;
		min-height: 26px;
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
	.folder .label {
		font-weight: 600;
	}
	.label {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.method {
		flex: none;
		width: 44px;
		padding: 3px 0;
		text-align: center;
		border-radius: 4px;
		background: color-mix(in srgb, currentColor 13%, transparent);
	}
	.actions {
		display: flex;
		gap: 1px;
		width: 0;
		overflow: hidden;
		opacity: 0;
		transition: opacity 0.12s;
	}
	.row:hover .actions,
	.row:focus-within .actions {
		width: auto;
		opacity: 1;
	}
	.run:hover:not(:disabled) {
		color: var(--ok);
	}
	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		flex: none;
	}
	.dot.passed {
		background: var(--ok);
		box-shadow: 0 0 0 3px var(--ok-bg);
	}
	.dot.failed,
	.dot.error {
		background: var(--fail);
		box-shadow: 0 0 0 3px var(--fail-bg);
	}
	.dot.cancelled {
		background: var(--muted);
	}
	.dot.running {
		background: var(--warn);
		animation: pulse 0.8s infinite alternate;
	}
	.err {
		color: var(--fail);
		display: inline-flex;
	}
	@keyframes pulse {
		to {
			opacity: 0.3;
		}
	}
</style>
