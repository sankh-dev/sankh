<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { api } from './api';
	import Icon from './Icon.svelte';
	import SecretInput from './SecretInput.svelte';
	import { STATUS_TEXT, isSecretName, setEnvVar, setLocalVar, type VarInfo } from './vars';

	interface Props {
		cid: string;
		/** Selected environment of the collection; empty when it has none. */
		env: string;
		info: VarInfo;
		/** Where to show the popover, below this box. */
		anchor: DOMRect;
		onsaved: () => Promise<void>;
		onmanage: () => void;
		onclose: () => void;
	}

	let { cid, env, info, anchor, onsaved, onmanage, onclose }: Props = $props();

	type Target = 'env' | 'local';

	const WIDTH = 340;
	const name = $derived(info.name);
	let secret = $derived(isSecretName(name));
	let target = $state<Target>('local');
	let value = $state('');
	let original = $state('');
	let loading = $state(true);
	let busy = $state(false);
	let error = $state<string | null>(null);
	let box = $state<HTMLDivElement>();
	let input = $state<HTMLInputElement>();
	let left = $derived(Math.max(8, Math.min(anchor.left, window.innerWidth - WIDTH - 8)));
	let top = $derived(anchor.bottom + 260 > window.innerHeight ? Math.max(8, anchor.top - 230) : anchor.bottom + 6);

	async function load(next: Target) {
		target = next;
		loading = true;
		error = null;
		try {
			const { vars } = next === 'env' ? await api.envFile(cid, env) : await api.envLocal(cid);
			original = vars.find((v) => v.name === name)?.value ?? '';
			value = original;
		} catch (err) {
			error = err instanceof Error ? err.message : String(err);
		} finally {
			loading = false;
		}
		await tick();
		input?.focus();
		input?.select();
	}

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = null;
		try {
			if (target === 'env') await setEnvVar(cid, env, name, value);
			else await setLocalVar(cid, name, value);
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

	onMount(() => {
		load(env && !secret && info.source !== 'local' ? 'env' : 'local');
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
			<span class={['state', info.status]}>{STATUS_TEXT[info.status]}</span>
		</div>
		<div class="targets" role="radiogroup" aria-label="Save to">
			{#if env}
				<button
					type="button"
					role="radio"
					aria-checked={target === 'env'}
					class={['target', target === 'env' && 'active']}
					onclick={() => load('env')}
					disabled={busy}
				>
					<span class="mono">environments/{env}.env</span>
				</button>
			{/if}
			<button
				type="button"
				role="radio"
				aria-checked={target === 'local'}
				class={['target', target === 'local' && 'active']}
				onclick={() => load('local')}
				disabled={busy}
			>
				<span class="mono">.env.local</span>
			</button>
		</div>
		{#if secret}
			<SecretInput
				bind:value
				bind:input
				placeholder={loading ? 'loading…' : 'value'}
				disabled={loading}
				aria-label="Value of {name}"
			/>
		{:else}
			<input
				bind:this={input}
				class="mono"
				bind:value
				placeholder={loading ? 'loading…' : 'value'}
				disabled={loading}
				aria-label="Value of {name}"
			/>
		{/if}
		<p class="note">
			{#if target === 'env'}
				Committed with the collection; applies only to <span class="mono">{env}</span>.
				{#if secret}Keep secrets in <code>.env.local</code> instead.{/if}
				{#if info.source === 'local'}
					<span class="warn"><code>.env.local</code> also sets it and wins over this file.</span>
				{/if}
			{:else}
				Saved to <code>.env.local</code> (gitignored), so it applies to every environment.
			{/if}
		</p>
		{#if error}<p class="error">{error}</p>{/if}
		<div class="buttons">
			<button type="button" class="ghost link" onclick={onmanage}>Environments…</button>
			<span class="spacer"></span>
			<button type="button" onclick={onclose}>Cancel</button>
			<button type="submit" class="primary" disabled={busy || loading || value === original}>
				{busy ? 'Saving…' : 'Save'}
			</button>
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
	.state.ok {
		color: var(--ok);
	}
	.state.runtime {
		color: var(--info);
	}
	.state.missing {
		color: var(--fail);
	}
	.targets {
		display: flex;
		gap: 4px;
	}
	.target {
		flex: 1;
		min-width: 0;
		min-height: 24px;
		padding: 0 8px;
		font-size: 11px;
		background: none;
		color: var(--muted);
	}
	.target span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 11px;
	}
	.target.active {
		color: var(--text);
		background: var(--selected);
		border-color: var(--accent);
	}
	.note {
		margin: 0;
		font-size: 11px;
		color: var(--muted);
	}
	.warn {
		display: block;
		margin-top: 4px;
		color: var(--warn);
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
