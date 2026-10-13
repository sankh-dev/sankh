<script lang="ts">
	import Icon from './Icon.svelte';
	import SecretInput from './SecretInput.svelte';
	import { STATUS_TEXT, isSecretName, setLocalVar, type VarStatus } from './vars';

	interface Props {
		cid: string;
		name: string;
		status: VarStatus;
		/** Where to show the popover, below this box. */
		anchor: DOMRect;
		onsaved: () => Promise<void>;
		onmanage: () => void;
		onclose: () => void;
	}

	let { cid, name, status, anchor, onsaved, onmanage, onclose }: Props = $props();

	const WIDTH = 320;
	let value = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);
	let box = $state<HTMLDivElement>();
	let input = $state<HTMLInputElement>();
	let secret = $derived(isSecretName(name));
	let left = $derived(Math.max(8, Math.min(anchor.left, window.innerWidth - WIDTH - 8)));
	let top = $derived(anchor.bottom + 220 > window.innerHeight ? Math.max(8, anchor.top - 190) : anchor.bottom + 6);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = null;
		try {
			await setLocalVar(cid, name, value);
			await onsaved();
			onclose();
		} catch (err) {
			error = err instanceof Error ? err.message : String(err);
		} finally {
			busy = false;
		}
	}

	function onpointerdown(e: PointerEvent) {
		if (box && !box.contains(e.target as Node)) onclose();
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key !== 'Escape') return;
		e.preventDefault();
		e.stopPropagation();
		onclose();
	}

	$effect(() => {
		input?.focus();
	});
</script>

<svelte:window {onpointerdown} {onkeydown} />

<div
	bind:this={box}
	class="pop"
	role="dialog"
	style:left="{left}px"
	style:top="{top}px"
	style:width="{WIDTH}px"
	aria-label="Set {name}"
>
	<form onsubmit={submit}>
		<div class="head">
			<span class="mono name">{#if secret}<Icon name="lock" size={11} />{/if}{name}</span>
			<span class={['state', status]}>{STATUS_TEXT[status]}</span>
		</div>
		{#if secret}
			<SecretInput bind:value bind:input placeholder="value" aria-label="Value of {name}" />
		{:else}
			<input bind:this={input} class="mono" bind:value placeholder="value" aria-label="Value of {name}" />
		{/if}
		<p class="note">Saved to <code>.env.local</code> (gitignored), so it applies to every environment.</p>
		{#if error}<p class="error">{error}</p>{/if}
		<div class="buttons">
			<button type="button" class="ghost link" onclick={onmanage}>Environments…</button>
			<span class="spacer"></span>
			<button type="button" onclick={onclose}>Cancel</button>
			<button type="submit" class="primary" disabled={busy}>{busy ? 'Saving…' : 'Save'}</button>
		</div>
	</form>
</div>

<style>
	.pop {
		position: fixed;
		z-index: 30;
		padding: 12px;
		background: var(--panel-2);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius);
		box-shadow: var(--shadow);
		animation: pop 0.1s ease-out;
	}
	form {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}
	.name {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		font-weight: 600;
		color: var(--text);
	}
	.state {
		font-size: 11px;
		color: var(--warn);
	}
	.state.missing {
		color: var(--fail);
	}
	.note {
		margin: 0;
		font-size: 11px;
		color: var(--muted);
	}
	.error {
		margin: 0;
		font-size: 12px;
		color: var(--fail);
	}
	.buttons {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.link {
		padding: 0 6px;
		font-size: 12px;
	}
	.spacer {
		flex: 1;
	}
	@keyframes pop {
		from {
			opacity: 0;
			transform: translateY(-3px);
		}
	}
</style>
