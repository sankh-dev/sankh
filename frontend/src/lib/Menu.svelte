<script module lang="ts">
	import type { IconName } from './Icon.svelte';

	export interface MenuItem {
		label: string;
		icon?: IconName;
		danger?: boolean;
		disabled?: boolean;
		/** Shown dimmed after the label. */
		hint?: string;
		onselect: () => void;
	}
</script>

<script lang="ts">
	import { tick } from 'svelte';
	import Icon from './Icon.svelte';

	interface Props {
		items: MenuItem[];
		/** Accessible name and tooltip of the trigger. */
		title: string;
		/** Visible trigger text; icon-only when absent. */
		label?: string;
		icon?: IconName;
		align?: 'left' | 'right';
		/** Extra class on the trigger button. */
		class?: string;
	}

	let { items, title, label, icon = 'more', align = 'right', class: className }: Props = $props();

	let open = $state(false);
	let pos = $state({ top: 0, left: 0, right: 0 });
	let trigger = $state<HTMLButtonElement>();
	let list = $state<HTMLDivElement>();

	function buttons() {
		return Array.from(list?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? []);
	}

	async function show() {
		const r = trigger!.getBoundingClientRect();
		pos = { top: r.bottom + 4, left: r.left, right: window.innerWidth - r.right };
		open = true;
		await tick();
		buttons()[0]?.focus();
	}

	function hide(refocus = true) {
		if (!open) return;
		open = false;
		if (refocus) trigger?.focus();
	}

	function choose(item: MenuItem) {
		hide();
		item.onselect();
	}

	function onkeydown(e: KeyboardEvent) {
		const all = buttons();
		const i = all.indexOf(document.activeElement as HTMLButtonElement);
		if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			e.preventDefault();
			const step = e.key === 'ArrowDown' ? 1 : -1;
			all[(i + step + all.length) % all.length]?.focus();
		} else if (e.key === 'Home' || e.key === 'End') {
			e.preventDefault();
			all[e.key === 'Home' ? 0 : all.length - 1]?.focus();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			hide();
		} else if (e.key === 'Tab') {
			hide(false);
		}
	}

	function closeOnScroll(node: HTMLElement) {
		const onscroll = (e: Event) => !node.contains(e.target as Node) && hide(false);
		document.addEventListener('scroll', onscroll, true);
		return () => document.removeEventListener('scroll', onscroll, true);
	}

	function onpointerdown(e: PointerEvent) {
		const t = e.target as Node;
		if (open && !list?.contains(t) && !trigger?.contains(t)) hide(false);
	}
</script>

<svelte:window {onpointerdown} onresize={() => hide(false)} onblur={() => hide(false)} />

<button
	bind:this={trigger}
	class={[label ? 'ghost' : 'icon-btn', className]}
	{title}
	aria-label={title}
	aria-haspopup="menu"
	aria-expanded={open}
	onclick={() => (open ? hide() : show())}
	onkeydown={(e) => e.key === 'ArrowDown' && !open && (e.preventDefault(), show())}
>
	<Icon name={icon} />
	{#if label}<span>{label}</span>{/if}
</button>

{#if open}
	<div
		bind:this={list}
		{@attach closeOnScroll}
		class="menu"
		role="menu"
		tabindex="-1"
		aria-label={title}
		style:top="{pos.top}px"
		style:left={align === 'left' ? `${pos.left}px` : null}
		style:right={align === 'right' ? `${pos.right}px` : null}
		{onkeydown}
	>
		{#each items as item (item.label)}
			<button
				role="menuitem"
				class={['item', item.danger && 'danger']}
				disabled={item.disabled}
				onclick={() => choose(item)}
			>
				{#if item.icon}<Icon name={item.icon} />{:else}<span class="pad"></span>{/if}
				<span class="label">{item.label}</span>
				{#if item.hint}<span class="hint">{item.hint}</span>{/if}
			</button>
		{/each}
	</div>
{/if}

<style>
	.menu {
		position: fixed;
		z-index: 20;
		min-width: 200px;
		padding: 4px;
		display: flex;
		flex-direction: column;
		background: var(--panel-2);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius);
		box-shadow: var(--shadow);
		animation: pop 0.1s ease-out;
	}
	.item {
		justify-content: flex-start;
		gap: 10px;
		width: 100%;
		background: none;
		border: none;
		border-radius: var(--radius-sm);
		padding: 0 10px;
		min-height: 30px;
		text-align: left;
		color: var(--text);
		white-space: nowrap;
	}
	.item:hover:not(:disabled),
	.item:focus-visible {
		background: var(--panel-3);
		outline: none;
	}
	.item.danger {
		color: var(--fail);
	}
	.item.danger:hover:not(:disabled),
	.item.danger:focus-visible {
		background: var(--fail-bg);
	}
	.pad {
		width: 14px;
	}
	.label {
		flex: 1;
	}
	.hint {
		color: var(--muted);
		font-size: 11px;
	}
	@keyframes pop {
		from {
			opacity: 0;
			transform: translateY(-3px);
		}
	}
</style>
