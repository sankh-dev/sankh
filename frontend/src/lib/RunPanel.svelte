<script lang="ts">
	import Icon from './Icon.svelte';
	import type { RunState } from './types';

	interface Props {
		run: RunState;
		/** Collection name. */
		name: string;
		selected: string | null;
		onpick: (path: string) => void;
	}

	let { run, name, selected, onpick }: Props = $props();
	let done = $derived(Object.keys(run.results).length);
</script>

<div class="panel">
	<div class="head">
		<strong class="title">Run {name}{run.target ? ` / ${run.target}` : ''}</strong>
		{#if run.summary}
			<span class="pill ok">{run.summary.passed} passed</span>
			{#if run.summary.failed}<span class="pill bad">{run.summary.failed} failed</span>{/if}
			{#if run.summary.errors}<span class="pill bad">{run.summary.errors} errors</span>{/if}
			<span class="muted">{run.summary.duration_ms.toFixed(0)} ms</span>
		{:else}
			<span class="muted">{done}/{run.paths.length}</span>
		{/if}
	</div>
	{#if !run.summary && run.paths.length}
		<div class="progress"><span style:width="{(done / run.paths.length) * 100}%"></span></div>
	{/if}
	<ul>
		{#each run.paths as path (path)}
			{@const r = run.results[path]}
			<li>
				<button class={{ selected: selected === path }} onclick={() => onpick(path)}>
					<span class="mark {r?.outcome ?? (run.current === path ? 'running' : 'pending')}">
						{#if r}
							<Icon name={r.outcome === 'passed' ? 'check' : 'x'} size={12} />
						{:else if run.current === path}
							<span class="spinner"></span>
						{:else}
							<span class="pip"></span>
						{/if}
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
		gap: 8px;
		align-items: center;
		height: 40px;
		padding: 0 12px;
		background: var(--panel);
		border-bottom: 1px solid var(--border);
	}
	.title {
		margin-right: auto;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.pill {
		font-size: 11px;
		font-weight: 600;
		padding: 2px 8px;
		border-radius: 999px;
	}
	.pill.ok {
		background: var(--ok-bg);
	}
	.pill.bad {
		background: var(--fail-bg);
	}
	.progress {
		height: 2px;
		background: var(--border);
	}
	.progress span {
		display: block;
		height: 100%;
		background: var(--accent);
		transition: width 0.2s;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 4px;
		overflow: auto;
	}
	li button {
		width: 100%;
		justify-content: flex-start;
		gap: 8px;
		min-height: 26px;
		background: none;
		border: none;
		text-align: left;
		padding: 0 8px;
	}
	li button:hover:not(:disabled),
	li button.selected {
		background: var(--panel-2);
	}
	li button.selected {
		box-shadow: inset 2px 0 0 var(--accent);
	}
	.mark {
		width: 12px;
		display: inline-flex;
		justify-content: center;
		flex: none;
	}
	.pip {
		width: 4px;
		height: 4px;
		border-radius: 50%;
		background: currentColor;
	}
	.spinner {
		width: 10px;
		height: 10px;
		border: 2px solid currentColor;
		border-right-color: transparent;
		border-radius: 50%;
		animation: spin 0.7s linear infinite;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
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
