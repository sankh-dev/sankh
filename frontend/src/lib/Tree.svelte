<script lang="ts">
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
			<span class="chev">{open ? '▾' : '▸'}</span>
			<span class="label">{node.name}</span>
		</button>
		<span class="actions">
			<button class="icon" title="New request here" onclick={() => onnew(node.path)}>+</button>
			<button class="icon" title="Run folder" onclick={() => onrun(node.path)}>▶</button>
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
			{#if running === node.path}
				<span class="dot running" title="running"></span>
			{:else if outcomes[node.path]}
				<span class="dot {outcomes[node.path]}" title={outcomes[node.path]}></span>
			{/if}
			{#if node.errors > 0}
				<span class="err" title="{node.errors} annotation error(s)">!</span>
			{/if}
		</button>
		<span class="actions">
			<button class="icon" title="Run" onclick={() => onrun(node.path)}>▶</button>
		</span>
	</div>
{/if}

<style>
	.row {
		display: flex;
		align-items: center;
		padding-left: calc(var(--depth) * 12px + 6px);
		border-radius: 6px;
		margin: 1px 4px;
	}
	.row:hover {
		background: var(--panel-2);
	}
	.row.selected {
		background: #1b3a31;
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
	.folder .label {
		font-weight: 600;
	}
	.label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.chev {
		width: 10px;
		color: var(--muted);
	}
	.method {
		width: 38px;
		flex: none;
	}
	.actions {
		display: none;
		gap: 2px;
		padding-right: 4px;
	}
	.row:hover .actions {
		display: flex;
	}
	.icon {
		padding: 0 6px;
		font-size: 11px;
		line-height: 18px;
		background: none;
	}
	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		flex: none;
		margin-left: auto;
	}
	.dot.passed {
		background: var(--ok);
	}
	.dot.failed,
	.dot.error {
		background: var(--fail);
	}
	.dot.running {
		background: var(--warn);
		animation: pulse 0.8s infinite alternate;
	}
	.err {
		color: var(--fail);
		font-weight: 700;
	}
	@keyframes pulse {
		to {
			opacity: 0.3;
		}
	}
</style>
