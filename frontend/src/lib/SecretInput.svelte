<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';
	import Icon from './Icon.svelte';

	interface Props extends Omit<HTMLInputAttributes, 'value' | 'type' | 'class'> {
		value: string;
		/** Shown as plain text. */
		revealed?: boolean;
		/** Masks again when focus leaves the field. */
		autohide?: boolean;
		/** Offers a copy button on hover. */
		copyable?: boolean;
		class?: string;
		input?: HTMLInputElement;
	}

	let {
		value = $bindable(),
		revealed = $bindable(false),
		autohide = false,
		copyable = false,
		class: className,
		input = $bindable(),
		...rest
	}: Props = $props();

	let copied = $state(false);

	async function copy() {
		await navigator.clipboard.writeText(value);
		copied = true;
		setTimeout(() => (copied = false), 1000);
	}

	function onfocusout(e: FocusEvent) {
		const next = e.relatedTarget as Node | null;
		if (autohide && !(e.currentTarget as HTMLElement).contains(next)) revealed = false;
	}
</script>

<div class={['secret', copyable && 'copyable']} {onfocusout}>
	<input
		bind:this={input}
		class={['mono', className]}
		type={revealed ? 'text' : 'password'}
		autocomplete="off"
		spellcheck="false"
		bind:value
		{...rest}
	/>
	<span class="adorn">
		{#if copyable}
			<button
				type="button"
				class="icon-btn copy"
				title={copied ? 'Copied' : 'Copy value'}
				aria-label="Copy value"
				disabled={!value}
				onclick={copy}
			>
				<Icon name={copied ? 'check' : 'copy'} size={12} />
			</button>
		{/if}
		<button
			type="button"
			class="icon-btn"
			title={revealed ? 'Hide value' : 'Show value'}
			aria-label={revealed ? 'Hide value' : 'Show value'}
			aria-pressed={revealed}
			onclick={() => (revealed = !revealed)}
		>
			<Icon name={revealed ? 'eye-off' : 'eye'} size={13} />
		</button>
	</span>
</div>

<style>
	.secret {
		position: relative;
		display: flex;
		min-width: 0;
	}
	input {
		flex: 1;
		width: 100%;
		padding-right: 30px;
	}
	.copyable input {
		padding-right: 54px;
	}
	.adorn {
		position: absolute;
		top: 0;
		bottom: 0;
		right: 2px;
		display: flex;
		align-items: center;
		gap: 0;
	}
	.adorn .icon-btn {
		width: 22px;
		min-height: 22px;
	}
	.copy {
		opacity: 0;
		transition: opacity 0.12s;
	}
	.secret:hover .copy,
	.secret:focus-within .copy {
		opacity: 1;
	}
</style>
