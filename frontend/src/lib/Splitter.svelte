<script lang="ts">
	/** A vertical drag handle that resizes the column to its left. */
	interface Props {
		label: string;
		/** Current width in px of the column being resized. */
		value: number;
		min: number;
		max: number;
		onchange: (value: number) => void;
		onreset: () => void;
		class?: string;
	}

	let { label, value, min, max, onchange, onreset, class: cls = '' }: Props = $props();

	let start: { x: number; value: number } | null = $state(null);

	function clamp(v: number) {
		return Math.round(Math.max(min, Math.min(Math.max(min, max), v)));
	}

	function onpointerdown(e: PointerEvent) {
		if (e.button !== 0) return;
		e.preventDefault();
		e.currentTarget instanceof HTMLElement && e.currentTarget.setPointerCapture(e.pointerId);
		start = { x: e.clientX, value };
	}

	function onpointermove(e: PointerEvent) {
		if (start) onchange(clamp(start.value + e.clientX - start.x));
	}

	function onkeydown(e: KeyboardEvent) {
		const step = e.shiftKey ? 50 : 10;
		if (e.key === 'ArrowLeft') onchange(clamp(value - step));
		else if (e.key === 'ArrowRight') onchange(clamp(value + step));
		else if (e.key === 'Home') onchange(clamp(min));
		else if (e.key === 'End') onchange(clamp(max));
		else return;
		e.preventDefault();
	}
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
	class="splitter {cls}"
	class:dragging={start !== null}
	role="separator"
	aria-orientation="vertical"
	aria-label={label}
	aria-valuenow={Math.round(value)}
	aria-valuemin={Math.round(min)}
	aria-valuemax={Math.round(Math.max(min, max))}
	tabindex="0"
	title="Drag to resize · double-click to reset"
	{onpointerdown}
	{onpointermove}
	onpointerup={() => (start = null)}
	onpointercancel={() => (start = null)}
	ondblclick={onreset}
	{onkeydown}
></div>

<style>
	.splitter {
		--line: var(--border);
		--line-w: 1px;
		--line-x: 2px;
		cursor: col-resize;
		touch-action: none;
		outline: none;
		min-height: 0;
		background: linear-gradient(var(--line), var(--line)) var(--line-x) 0 / var(--line-w) 100% no-repeat;
	}
	.splitter:hover,
	.splitter:focus-visible,
	.splitter.dragging {
		--line: var(--accent);
		--line-w: 2px;
		--line-x: 1px;
	}
</style>
