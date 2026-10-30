<script lang="ts">
	import Icon from './Icon.svelte';
	import ResponseBody from './ResponseBody.svelte';
	import { formatSize } from './format';
	import type { AnnotationKind } from './jsonpath';
	import type { ErrorKind, RequestResult } from './types';

	interface Props {
		result: RequestResult | null;
		/** Some request in the current run skipped TLS certificate checks. */
		runInsecure?: boolean;
		/** Adds an `@expect` / `@capture` line to the open request. */
		onadd?: (kind: AnnotationKind, line: string) => void;
	}

	let { result, runInsecure = false, onadd }: Props = $props();
	let tab = $state<'body' | 'headers' | 'request'>('body');
	let shownTab = $derived(result?.response ? tab : 'request');

	const ERROR_KINDS: Partial<Record<ErrorKind, string>> = {
		transport: 'Connection error',
		parse: 'Cannot read request',
		config: 'Configuration error'
	};

	function statusClass(status: number) {
		if (status < 300) return 'ok';
		if (status < 400) return 'redirect';
		if (status < 500) return 'client';
		return 'bad';
	}
</script>

{#snippet headerTable(headers: [string, string][])}
	<table>
		<tbody>
			{#each headers as [k, v], i (i)}
				<tr><th class="mono">{k}</th><td class="mono">{v}</td></tr>
			{/each}
		</tbody>
	</table>
{/snippet}

<div class="pane">
	{#if !result}
		<div class="empty">
			<Icon name="play" size={20} />
			<p>Run a request to see the response.</p>
		</div>
	{:else}
		<div class="summary">
			<span class="outcome {result.outcome}">{result.outcome}</span>
			{#if result.response}
				{@const res = result.response}
				<span class="status {statusClass(res.status)}">{res.status}</span>
				<span class="meta mono">{res.version}</span>
				<span class="meta">
					{res.time_ms.toFixed(0)} ms{#if res.ttfb_ms !== undefined}&nbsp;(TTFB {res.ttfb_ms.toFixed(0)} ms){/if}
				</span>
				<span class="meta">{formatSize(res.size)}</span>
				<span class="meta url mono" title={res.url}>{res.url}</span>
			{/if}
		</div>

		{#if result.insecure || runInsecure}
			<div class="insecure" role="status">
				<Icon name="alert" size={12} />
				{result.insecure
					? 'TLS certificate verification is disabled for this request (-k).'
					: 'TLS certificate verification is disabled for some requests in this run (-k).'}
			</div>
		{/if}

		{#if result.error}
			<div class="error">
				{#if result.outcome === 'error' && result.error_kind && ERROR_KINDS[result.error_kind]}
					<strong class="kind">{ERROR_KINDS[result.error_kind]}</strong>
				{/if}
				<pre>{result.error}</pre>
			</div>
		{/if}

		{#if result.assertions.length || result.warnings.length || result.captures.length || result.diagnostics?.length}
			<ul class="checks">
				{#each result.diagnostics ?? [] as d, i (i)}
					<li class={d.severity === 'error' ? 'fail' : 'warn'}>
						<span class="mark">{#if d.severity === 'error'}✗{:else}<Icon name="alert" size={12} />{/if}</span>
						{#if d.line}<code class="pos">{d.line}{d.column ? `:${d.column}` : ''}</code>{/if}
						<span class="msg">{d.message}{d.help ? ` (${d.help})` : ''}</span>
					</li>
				{/each}
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
					<li class="warn"><span class="mark"><Icon name="alert" size={12} /></span>{w}</li>
				{/each}
			</ul>
		{/if}

		{#if result.response || result.request}
			{@const res = result.response}
			{@const req = result.request}
			<div class="tabs" role="tablist">
				{#if res}
					<button role="tab" aria-selected={shownTab === 'body'} class={{ active: shownTab === 'body' }} onclick={() => (tab = 'body')}>Body</button>
					<button role="tab" aria-selected={shownTab === 'headers'} class={{ active: shownTab === 'headers' }} onclick={() => (tab = 'headers')}>
						Headers <span class="count">{res.headers.length}</span>
					</button>
				{/if}
				{#if req || res?.redirects}
					<button role="tab" aria-selected={shownTab === 'request'} class={{ active: shownTab === 'request' }} onclick={() => (tab = 'request')}>
						Request
						{#if res?.redirects}<span class="count" title="Redirects followed">{res.redirects.length}</span>{/if}
					</button>
				{/if}
			</div>
			<div class="content">
				{#if shownTab === 'body' && res}
					{#if res.body_capped}
						<p class="notice">
							The body is larger than the <code>max_body</code> limit (set in <code>sankh.toml</code>); only the
							first part is shown.
						</p>
					{/if}
					{#if res.sha256}<p class="meta">SHA-256 <code class="digest">{res.sha256}</code></p>{/if}
					<ResponseBody response={res} {onadd} />
					{#if res.body_truncated && !res.body_capped}<p class="meta">Body truncated for display.</p>{/if}
				{:else if shownTab === 'headers' && res}
					{@render headerTable(res.headers)}
				{:else}
					{#if req}
						<p class="line mono">
							<span class="method method-{req.method}">{req.method}</span>
							<span class="req-url">{req.url}</span>
						</p>
						{@render headerTable(req.headers)}
						{#if req.body !== undefined}
							<h4>Body <span class="meta">{formatSize(req.body_size)}</span></h4>
							<pre class="mono req-body">{req.body}</pre>
						{:else if req.body_size}
							<p class="meta">Body: {formatSize(req.body_size)} (not shown)</p>
						{/if}
					{/if}
					{#if res?.redirects}
						<h4>Redirects</h4>
						<ol class="redirects mono">
							{#each res.redirects as hop, i (i)}
								<li>
									<span class="status {statusClass(hop.status)}">{hop.status}</span>
									<span class="hop" title={hop.url}>{hop.url}</span>
									<span class="arrow">→</span>
									<span class="hop" title={hop.location}>{hop.location}</span>
								</li>
							{/each}
						</ol>
					{/if}
				{/if}
			</div>
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
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		color: var(--muted);
		opacity: 0.8;
	}
	.empty p {
		margin: 0;
	}
	.summary {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 44px;
		padding: 0 12px;
		border-bottom: 1px solid var(--border);
		background: var(--panel);
		min-width: 0;
	}
	.outcome {
		text-transform: uppercase;
		font: 700 10px/1 var(--sans);
		letter-spacing: 0.04em;
		padding: 5px 8px;
		border-radius: 999px;
	}
	.outcome.passed {
		background: var(--ok-bg);
		color: var(--ok);
	}
	.outcome.failed,
	.outcome.error {
		background: var(--fail-bg);
		color: var(--fail);
	}
	.outcome.cancelled {
		background: var(--warn-bg);
		color: var(--warn);
	}
	.status {
		font: 700 12px/1 var(--mono);
		padding: 4px 8px;
		border-radius: var(--radius-sm);
		background: color-mix(in srgb, currentColor 14%, transparent);
	}
	.status.ok {
		color: var(--ok);
	}
	.status.redirect {
		color: var(--info);
	}
	.status.client {
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
		padding: 8px 12px;
		color: var(--fail);
		border-bottom: 1px solid var(--border);
	}
	.error .kind {
		display: block;
		margin-bottom: 2px;
		font-size: 12px;
	}
	.insecure,
	.notice {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 12px;
		background: var(--warn-bg);
		color: #f8e3a8;
		font-size: 12px;
	}
	.insecure {
		border-bottom: 1px solid var(--warn-border);
	}
	.insecure :global(.icon) {
		color: var(--warn);
	}
	.notice {
		display: block;
		margin: 0 0 8px;
		border: 1px solid var(--warn-border);
		border-radius: var(--radius-sm);
	}
	.pos {
		color: var(--muted);
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
		align-self: center;
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
	.tabs button:hover:not(:disabled) {
		background: none;
		color: var(--text);
	}
	.tabs .active {
		color: var(--text);
		border-bottom-color: var(--accent);
	}
	.count {
		color: var(--muted);
		font-size: 10px;
		padding: 1px 6px;
		border-radius: 999px;
		background: var(--panel-3);
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
	.content > .meta {
		margin: 0 0 8px;
	}
	.digest {
		color: var(--text);
		word-break: break-all;
	}
	.line {
		display: flex;
		gap: 8px;
		align-items: baseline;
		margin: 0 0 8px;
	}
	.req-url {
		word-break: break-all;
	}
	h4 {
		margin: 12px 0 4px;
		font-size: 11px;
		font-weight: 600;
		color: var(--muted);
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}
	h4 .meta {
		text-transform: none;
		letter-spacing: 0;
		font-weight: 400;
	}
	.req-body {
		padding: 6px 8px;
		background: var(--panel);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	.redirects {
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.redirects li {
		display: flex;
		gap: 8px;
		align-items: center;
		padding: 2px 0;
		min-width: 0;
	}
	.redirects .status {
		padding: 2px 6px;
		font-size: 11px;
	}
	.hop {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}
	.arrow {
		color: var(--muted);
		flex: none;
	}
</style>
