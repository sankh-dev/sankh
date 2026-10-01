<script lang="ts">
	import type { RunState } from './types';

	interface Props {
		run: RunState;
		selected: string | null;
		onpick: (path: string) => void;
	}

	let { run, selected, onpick }: Props = $props();
	let done = $derived(Object.keys(run.results).length);
</script>

<div class="panel">
	<div class="head">
		<strong>Run {run.target || 'collection'}</strong>
		{#if run.summary}
			<span class="ok">{run.summary.passed} passed</span>
			{#if run.summary.failed}<span class="bad">{run.summary.failed} failed</span>{/if}
			{#if run.summary.errors}<span class="bad">{run.summary.errors} errors</span>{/if}
			<span class="muted">{run.summary.duration_ms.toFixed(0)} ms</span>
		{:else}
			<span class="muted">{done}/{run.paths.length}</span>
		{/if}
	</div>
	<ul>
		{#each run.paths as path (path)}
			{@const r = run.results[path]}
			<li>
				<button class={{ selected: selected === path }} onclick={() => onpick(path)}>
					<span class="mark {r?.outcome ?? (run.current === path ? 'running' : 'pending')}">
						{#if r}{r.outcome === 'passed' ? '✓' : '✗'}{:else if run.current === path}…{:else}·{/if}
					</span>
					<span class="name">{r?.name ?? path}</span>
					{#if r?.response}<span class="muted mono">{r.response.status}</span>{/if}
					<span class="muted mono path">{path}</span>
				</button>
			</li>
		{/each}
	</ul>
</div>

<style>
	.panel {
		border-bottom: 1px solid var(--border);
		max-height: 40%;
		display: flex;
		flex-direction: column;
	}
	.head {
		display: flex;
		gap: 10px;
		align-items: center;
		padding: 8px 12px;
		background: var(--panel);
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 4px;
		overflow: auto;
	}
	li button {
		width: 100%;
		display: flex;
		gap: 8px;
		align-items: center;
		background: none;
		border: none;
		text-align: left;
		padding: 3px 8px;
	}
	li button:hover,
	li button.selected {
		background: var(--panel-2);
	}
	.mark {
		width: 12px;
		font-weight: 700;
	}
	.passed,
	.ok {
		color: var(--ok);
	}
	.failed,
	.error,
	.bad {
		color: var(--fail);
	}
	.running {
		color: var(--warn);
	}
	.muted,
	.pending {
		color: var(--muted);
	}
	.path {
		margin-left: auto;
		font-size: 11px;
	}
</style>
