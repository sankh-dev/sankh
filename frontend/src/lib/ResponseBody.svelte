<script lang="ts">
	import AnnotatePopover from './AnnotatePopover.svelte';
	import Icon from './Icon.svelte';
	import JsonTree from './JsonTree.svelte';
	import { bodyKind, formatXml, mediaType, prettyJson } from './body';
	import type { AnnotationKind, JsonPick } from './jsonpath';
	import type { ResponseView } from './types';

	interface Props {
		response: ResponseView;
		/** Adds an `@expect` / `@capture` line to the open request; enables pick buttons in the JSON tree. */
		onadd?: (kind: AnnotationKind, line: string) => void;
	}

	let { response, onadd }: Props = $props();
	let raw = $state(false);
	let natural = $state<{ width: number; height: number } | null>(null);
	let picked = $state<JsonPick | null>(null);
	let tree = $state<ReturnType<typeof JsonTree>>();

	let kind = $derived(bodyKind(response.headers, response.body));
	let parsed = $derived.by(() => {
		if (kind !== 'json' || response.body_truncated) return undefined;
		try {
			return { value: JSON.parse(response.body) as unknown };
		} catch {
			return undefined;
		}
	});
	let formatted = $derived.by(() => {
		if (kind === 'json') return prettyJson(response.body);
		if (kind === 'xml') return formatXml(response.body);
		return null;
	});
	let imageSrc = $derived(
		response.body_base64 ? `data:${mediaType(response.headers)};base64,${response.body_base64}` : null
	);
	let previewLabel = $derived(kind === 'image' || kind === 'html' ? 'Preview' : 'Pretty');
	let hasPreview = $derived(
		kind === 'html' || (kind === 'image' && imageSrc !== null) || formatted !== null
	);
	let showRaw = $derived(raw || !hasPreview);

	function imageLoaded(e: Event) {
		const img = e.currentTarget as HTMLImageElement;
		natural = { width: img.naturalWidth, height: img.naturalHeight };
	}
</script>

<div class="body">
	{#if kind !== 'text'}
		<div class="toolbar">
			<div class="toggle" role="group" aria-label="Body view">
				<button class={{ active: !showRaw }} disabled={!hasPreview} onclick={() => (raw = false)}>
					{previewLabel}
				</button>
				<button class={{ active: showRaw }} onclick={() => (raw = true)}>Raw</button>
			</div>
			{#if parsed && !showRaw}
				<div class="folds" role="group" aria-label="Expand or collapse JSON">
					<button title="Expand all" aria-label="Expand all" onclick={() => tree?.expandAll()}>
						<Icon name="expand" size={12} />
					</button>
					<button title="Collapse all" aria-label="Collapse all" onclick={() => tree?.collapseAll()}>
						<Icon name="collapse" size={12} />
					</button>
					{#each [1, 2, 3] as level (level)}
						<button
							class="mono"
							title="Expand {level} level{level > 1 ? 's' : ''} deep"
							aria-label="Expand {level} level{level > 1 ? 's' : ''} deep"
							onclick={() => tree?.expandTo(level + 1)}>{level}</button
						>
					{/each}
				</div>
			{/if}
			{#if kind === 'image' && !showRaw && natural}
				<span class="meta">{natural.width} × {natural.height}</span>
			{/if}
			{#if kind === 'image' && !imageSrc}<span class="meta">Image too large to preview.</span>{/if}
			{#if kind === 'html' && !showRaw}
				<span class="meta">Sandboxed: scripts and external resources are blocked.</span>
			{/if}
		</div>
	{/if}

	{#if showRaw}
		<pre class="mono">{response.body}</pre>
	{:else if kind === 'image'}
		<div class="image">
			<!-- An SVG with only a viewBox has no intrinsic width and collapses to 0 in this shrink-to-fit box. -->
			<img
				src={imageSrc}
				alt="Response"
				onload={imageLoaded}
				style:width={natural ? `${natural.width}px` : undefined}
			/>
		</div>
	{:else if kind === 'html'}
		<iframe sandbox="" srcdoc={response.body} title="HTML preview" referrerpolicy="no-referrer"></iframe>
	{:else if parsed}
		<JsonTree bind:this={tree} data={parsed.value} onpick={onadd ? (p) => (picked = p) : undefined} />
	{:else}
		<pre class="mono">{formatted}</pre>
	{/if}
</div>

{#if picked && onadd}
	{#key picked}
		<AnnotatePopover pick={picked} {onadd} onclose={() => (picked = null)} />
	{/key}
{/if}

<style>
	.body {
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-height: 100%;
	}
	.toolbar {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.toggle {
		display: flex;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		overflow: hidden;
	}
	.toggle button {
		border: none;
		border-radius: 0;
		padding: 3px 10px;
		font-size: 12px;
		background: none;
		color: var(--muted);
	}
	.toggle button.active {
		background: var(--panel-3);
		color: var(--text);
	}
	.folds {
		display: flex;
		gap: 2px;
	}
	.folds button {
		min-height: 22px;
		min-width: 22px;
		padding: 0 5px;
		font-size: 11px;
		border: none;
		background: none;
		color: var(--muted);
	}
	.folds button:hover:not(:disabled) {
		background: var(--panel-3);
		color: var(--text);
	}
	.meta {
		color: var(--muted);
		font-size: 12px;
	}
	pre {
		margin: 0;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.image {
		align-self: flex-start;
		max-width: 100%;
		background: repeating-conic-gradient(#8883 0 25%, transparent 0 50%) 0 0 / 16px 16px;
		border: 1px solid var(--border);
	}
	.image img {
		display: block;
		max-width: 100%;
		height: auto;
	}
	iframe {
		flex: 1;
		min-height: 320px;
		width: 100%;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		background: #fff;
	}
</style>
