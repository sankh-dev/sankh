<script lang="ts">
	import { untrack } from 'svelte';
	import type { CollectionInfo } from './types';

	interface Props {
		path: string;
		targets: CollectionInfo[];
		oncopy: (to: string, toPath: string) => Promise<void>;
		onclose: () => void;
	}

	let { path, targets, oncopy, onclose }: Props = $props();

	let to = $state(untrack(() => targets[0]?.id ?? ''));
	let toPath = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	let target = $derived(targets.some((t) => t.id === to) ? to : (targets[0]?.id ?? ''));
	let destPath = $derived(toPath.trim() || path);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		error = null;
		busy = true;
		try {
			await oncopy(target, destPath);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
	<form class="dialog" onsubmit={submit} aria-label="Copy to collection">
		<h3>Copy <code>{path}</code> to another collection</h3>
		<label>
			Collection
			<select bind:value={to}>
				{#each targets as t (t.id)}
					<option value={t.id}>{t.name}</option>
				{/each}
			</select>
		</label>
		<label>
			Path in that collection
			<input class="mono" bind:value={toPath} placeholder={path} />
		</label>
		<p class="muted">An existing file is never overwritten.</p>
		{#if error}<p class="error">{error}</p>{/if}
		<div class="buttons">
			<button type="button" onclick={onclose}>Cancel</button>
			<button type="submit" class="primary" disabled={busy || !target}>Copy</button>
		</div>
	</form>
</div>

<style>
	.dialog {
		width: min(480px, 92vw);
	}
	p {
		margin: 0;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		color: var(--muted);
	}
	label input,
	label select {
		color: var(--text);
	}
	.muted {
		color: var(--muted);
		font-size: 12px;
	}
	.error {
		color: var(--fail);
	}
</style>
