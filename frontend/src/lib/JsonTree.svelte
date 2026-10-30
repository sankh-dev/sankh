<script lang="ts">
	import { SvelteMap, SvelteSet } from 'svelte/reactivity';
	import Icon from './Icon.svelte';
	import { jqLiteral, jqPath, type AnnotationKind, type JsonPick, type Segment } from './jsonpath';

	interface Props {
		data: unknown;
		/** Shows Expect / Capture buttons on hover when set. */
		onpick?: (pick: JsonPick) => void;
	}

	let { data, onpick }: Props = $props();

	const PAGE = 100;

	// Fresh per response, so a new body opens fully expanded.
	// Nodes shallower than `limit` are open unless toggled; `null` opens every depth.
	let limit = $derived.by<number | null>(() => {
		void data;
		return null;
	});
	let opened = $derived.by(() => {
		void data;
		return new SvelteSet<string>();
	});
	let closed = $derived.by(() => {
		void data;
		return new SvelteSet<string>();
	});
	let shown = $derived.by(() => {
		void data;
		return new SvelteMap<string, number>();
	});

	function isContainer(v: unknown): v is object {
		return typeof v === 'object' && v !== null;
	}

	function entriesOf(v: object): [Segment, unknown][] {
		return Array.isArray(v) ? v.map((x, i) => [i, x]) : Object.entries(v);
	}

	function scalarKind(v: unknown) {
		if (v === null) return 'null';
		return typeof v;
	}

	function isOpen(path: string, depth: number) {
		if (closed.has(path)) return false;
		if (opened.has(path)) return true;
		return limit === null || depth < limit;
	}

	function setOpen(path: string, open: boolean) {
		(open ? closed : opened).delete(path);
		(open ? opened : closed).add(path);
	}

	function setSubtree(value: unknown, segments: Segment[], open: boolean) {
		if (!isContainer(value)) return;
		setOpen(jqPath(segments), open);
		for (const [k, v] of entriesOf(value)) setSubtree(v, [...segments, k], open);
	}

	/** Alt- or Shift-click applies to the whole subtree. */
	function toggle(e: MouseEvent, value: unknown, segments: Segment[], open: boolean) {
		if (e.altKey || e.shiftKey) setSubtree(value, segments, !open);
		else setOpen(jqPath(segments), !open);
	}

	/** Opens nodes shallower than `depth` (the root is depth 0); `null` opens everything. */
	export function expandTo(depth: number | null) {
		opened.clear();
		closed.clear();
		limit = depth;
	}

	export function expandAll() {
		expandTo(null);
	}

	/** Leaves only the top-level keys visible. */
	export function collapseAll() {
		expandTo(1);
	}

	function pick(kind: AnnotationKind, segments: Segment[], value: unknown, e: MouseEvent) {
		const anchor = (e.currentTarget as HTMLElement).getBoundingClientRect();
		onpick?.({ kind, segments, value, anchor });
	}
</script>

{#snippet node(key: Segment | null, value: unknown, segments: Segment[], depth: number, comma: boolean)}
	{@const path = jqPath(segments)}
	{@const entries = isContainer(value) ? entriesOf(value) : []}
	{@const open = isContainer(value) && entries.length > 0 && isOpen(path, depth)}
	{@const [openBr, closeBr] = Array.isArray(value) ? ['[', ']'] : ['{', '}']}
	<div class="row" style:--depth={depth} data-path={path}>
		{#if isContainer(value) && entries.length}
			<button
				class={['toggle', open && 'open']}
				onclick={(e) => toggle(e, value, segments, open)}
				title="Alt-click to {open ? 'collapse' : 'expand'} everything inside"
				aria-label="{open ? 'Collapse' : 'Expand'} {path}"
				aria-expanded={open}
			>
				<Icon name="chevron" size={10} />
			</button>
		{/if}
		{#if typeof key === 'string'}<span class="key">{JSON.stringify(key)}</span><span class="punct">: </span>{/if}
		{#if isContainer(value)}
			{#if open}
				<span class="punct">{openBr}</span>
			{:else if entries.length}
				<span class="punct">{openBr}</span><button class="summary" onclick={(e) => toggle(e, value, segments, false)}
					>{entries.length} {Array.isArray(value) ? 'items' : 'keys'}</button
				><span class="punct">{closeBr}{comma ? ',' : ''}</span>
			{:else}
				<span class="punct">{openBr}{closeBr}{comma ? ',' : ''}</span>
			{/if}
		{:else}
			<span class={['val', scalarKind(value)]}>{jqLiteral(value)}</span>{#if comma}<span class="punct">,</span>{/if}
		{/if}
		{#if onpick && segments.length}
			<span class="acts">
				<button
					class="act"
					title="Add @expect for {path}"
					aria-label="Add expect for {path}"
					onclick={(e) => pick('expect', segments, value, e)}
				>
					<Icon name="check" size={11} />Expect
				</button>
				<button
					class="act"
					title="Add @capture for {path}"
					aria-label="Add capture for {path}"
					onclick={(e) => pick('capture', segments, value, e)}
				>
					<Icon name="arrow" size={11} />Capture
				</button>
			</span>
		{/if}
	</div>
	{#if open}
		{@const limit = shown.get(path) ?? PAGE}
		{#each entries.slice(0, limit) as [k, v], i (k)}
			{@render node(Array.isArray(value) ? null : k, v, [...segments, k], depth + 1, i < entries.length - 1)}
		{/each}
		{#if entries.length > limit}
			<div class="row" style:--depth={depth + 1}>
				<button class="summary" onclick={() => shown.set(path, limit + PAGE)}>
					Show {Math.min(PAGE, entries.length - limit)} more of {entries.length - limit}
				</button>
			</div>
		{/if}
		<div class="row" style:--depth={depth}><span class="punct">{closeBr}{comma ? ',' : ''}</span></div>
	{/if}
{/snippet}

<div class="tree mono">
	{@render node(null, data, [], 0, false)}
</div>

<style>
	.tree {
		line-height: 1.55;
	}
	.row {
		position: relative;
		padding-left: calc(var(--depth) * 2ch + 16px);
		white-space: pre-wrap;
		word-break: break-word;
		border-radius: var(--radius-sm);
	}
	.row:hover {
		background: color-mix(in srgb, var(--panel-3) 60%, transparent);
	}
	.toggle {
		position: absolute;
		left: calc(var(--depth) * 2ch);
		top: 3px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 14px;
		height: 14px;
		padding: 0;
		border: none;
		background: none;
		color: var(--muted);
		user-select: none;
	}
	.toggle :global(svg) {
		transition: transform 0.1s;
	}
	.toggle.open :global(svg) {
		transform: rotate(90deg);
	}
	.key {
		color: var(--info);
	}
	.punct {
		color: var(--muted);
	}
	.val.string {
		color: var(--ok);
	}
	.val.number {
		color: var(--warn);
	}
	.val.boolean,
	.val.null {
		color: var(--accent);
	}
	.summary {
		padding: 0 4px;
		margin: 0 2px;
		border: none;
		background: var(--panel-3);
		color: var(--muted);
		font: inherit;
		font-size: 11px;
		user-select: none;
	}
	.acts {
		display: inline-flex;
		gap: 4px;
		margin-left: 10px;
		vertical-align: middle;
		opacity: 0;
		user-select: none;
	}
	.row:hover > .acts,
	.acts:focus-within {
		opacity: 1;
	}
	.act {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		min-height: 0;
		padding: 0 6px;
		font: 500 10px/16px var(--sans);
		color: var(--muted);
		background: var(--panel-2);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}
	.act:hover {
		color: var(--text);
		border-color: var(--accent);
	}
</style>
