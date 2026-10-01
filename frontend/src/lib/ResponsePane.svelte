<script lang="ts">
	import type { RequestResult } from './types';

	interface Props {
		result: RequestResult | null;
	}

	let { result }: Props = $props();
	let tab = $state<'body' | 'headers' | 'log'>('body');

	let pretty = $derived.by(() => {
		const body = result?.response?.body ?? '';
		try {
			return JSON.stringify(JSON.parse(body), null, 2);
		} catch {
			return body;
		}
	});

	function formatSize(n: number) {
		if (n < 1024) return `${n} B`;
		if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
		return `${(n / 1024 / 1024).toFixed(1)} MB`;
	}

	function statusClass(status: number) {
		if (status < 300) return 'ok';
		if (status < 400) return 'redirect';
		return 'bad';
	}
</script>

<div class="pane">
	{#if !result}
		<div class="empty">Run a request to see the response.</div>
	{:else}
		<div class="summary">
			<span class="outcome {result.outcome}">{result.outcome}</span>
			{#if result.response}
				<span class="status {statusClass(result.response.status)}">{result.response.status}</span>
				<span class="meta">{result.response.time_ms.toFixed(0)} ms</span>
				<span class="meta">{formatSize(result.response.size)}</span>
				<span class="meta url mono" title={result.response.url}>{result.response.url}</span>
			{/if}
		</div>

		{#if result.error}
			<pre class="error">{result.error}</pre>
		{/if}

		{#if result.assertions.length || result.warnings.length || result.captures.length}
			<ul class="checks">
				{#each result.assertions as a (a.label)}
					<li class={a.passed ? 'pass' : 'fail'}>
						<span class="mark">{a.passed ? '✓' : '✗'}</span>
						<code>{a.label}</code>
						{#if a.message}<span class="msg">{a.message}</span>{/if}
					</li>
				{/each}
				{#each result.captures as c (c.name)}
					<li class="capture"><span class="mark">→</span><code>{c.name}</code> = <code>{c.value}</code></li>
				{/each}
				{#each result.warnings as w (w)}
					<li class="warn"><span class="mark">!</span>{w}</li>
				{/each}
			</ul>
		{/if}

		{#if result.response}
			<div class="tabs" role="tablist">
				<button role="tab" aria-selected={tab === 'body'} class={{ active: tab === 'body' }} onclick={() => (tab = 'body')}>Body</button>
				<button role="tab" aria-selected={tab === 'headers'} class={{ active: tab === 'headers' }} onclick={() => (tab = 'headers')}>
					Headers <span class="count">{result.response.headers.length}</span>
				</button>
				<button role="tab" aria-selected={tab === 'log'} class={{ active: tab === 'log' }} onclick={() => (tab = 'log')}>Log</button>
			</div>
			<div class="content">
				{#if tab === 'body'}
					<pre class="mono">{pretty}</pre>
					{#if result.response.body_truncated}<p class="meta">Body truncated for display.</p>{/if}
				{:else if tab === 'headers'}
					<table>
						<tbody>
							{#each result.response.headers as [k, v], i (i)}
								<tr><th class="mono">{k}</th><td class="mono">{v}</td></tr>
							{/each}
						</tbody>
					</table>
				{:else}
					<pre class="mono">{result.stderr || 'No output.'}</pre>
				{/if}
			</div>
		{:else if result.stderr}
			<pre class="content mono">{result.stderr}</pre>
		{/if}
	{/if}
</div>

<style>
	.pane {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}
	.empty {
		margin: auto;
		color: var(--muted);
	}
	.summary {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 12px;
		border-bottom: 1px solid var(--border);
		background: var(--panel);
		min-width: 0;
	}
	.outcome {
		text-transform: uppercase;
		font: 700 10px/1 var(--sans);
		padding: 4px 6px;
		border-radius: 4px;
	}
	.outcome.passed {
		background: #12402e;
		color: var(--ok);
	}
	.outcome.failed,
	.outcome.error {
		background: #4a1c1c;
		color: var(--fail);
	}
	.status {
		font: 700 14px var(--mono);
	}
	.status.ok {
		color: var(--ok);
	}
	.status.redirect {
		color: var(--warn);
	}
	.status.bad {
		color: var(--fail);
	}
	.meta {
		color: var(--muted);
		font-size: 12px;
	}
	.url {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.error {
		margin: 0;
		padding: 8px 12px;
		color: var(--fail);
		white-space: pre-wrap;
		border-bottom: 1px solid var(--border);
	}
	.checks {
		list-style: none;
		margin: 0;
		padding: 6px 12px;
		border-bottom: 1px solid var(--border);
		max-height: 35%;
		overflow: auto;
	}
	.checks li {
		display: flex;
		gap: 8px;
		align-items: baseline;
		padding: 2px 0;
	}
	.mark {
		width: 12px;
		flex: none;
		font-weight: 700;
	}
	.pass .mark {
		color: var(--ok);
	}
	.fail .mark,
	.fail .msg {
		color: var(--fail);
	}
	.capture .mark {
		color: var(--accent);
	}
	.warn {
		color: var(--warn);
	}
	.tabs {
		display: flex;
		gap: 2px;
		padding: 6px 10px 0;
		border-bottom: 1px solid var(--border);
	}
	.tabs button {
		border: none;
		border-bottom: 2px solid transparent;
		border-radius: 0;
		background: none;
		color: var(--muted);
	}
	.tabs .active {
		color: var(--text);
		border-bottom-color: var(--accent);
	}
	.count {
		color: var(--muted);
		font-size: 11px;
	}
	.content {
		flex: 1;
		overflow: auto;
		padding: 8px 12px;
		margin: 0;
	}
	pre {
		margin: 0;
		white-space: pre-wrap;
		word-break: break-word;
	}
	table {
		border-collapse: collapse;
		width: 100%;
	}
	th {
		text-align: left;
		color: var(--muted);
		font-weight: 400;
		padding: 2px 12px 2px 0;
		white-space: nowrap;
		vertical-align: top;
	}
	td {
		word-break: break-all;
	}
</style>
