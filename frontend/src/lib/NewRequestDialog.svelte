<script lang="ts">
	import { api } from './api';

	interface Props {
		collection: string;
		folder: string;
		oncreate: (path: string, content: string) => Promise<void>;
		onclose: () => void;
	}

	let { collection, folder, oncreate, onclose }: Props = $props();

	let name = $state('New request');
	let fileName = $state('');
	let curl = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	let suggested = $derived(
		name
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, '-')
			.replace(/^-|-$/g, '') || 'request'
	);

	const TEMPLATE = (n: string) =>
		`#!/usr/bin/env bash\n# @name ${n}\n# @expect status 200\ncurl -sS "$BASE_URL/"\n`;

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		error = null;
		busy = true;
		try {
			let file = (fileName.trim() || suggested).replace(/\.sh$/, '') + '.sh';
			const path = folder ? `${folder}/${file}` : file;
			const content = curl.trim() ? (await api.importCurl(curl, name)).content : TEMPLATE(name);
			await oncreate(path, content);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
	<form class="dialog" onsubmit={submit} aria-label="New request">
		<h3>New request in {collection} <code>/{folder}</code></h3>
		<label>Name <input bind:value={name} /></label>
		<label>File name <input class="mono" bind:value={fileName} placeholder="{suggested}.sh" /></label>
		<label>
			Import from curl (optional)
			<textarea class="mono" rows="6" bind:value={curl} placeholder="curl 'https://api.example.com/users' -H 'Accept: application/json'"></textarea>
		</label>
		{#if error}<p class="error">{error}</p>{/if}
		<div class="buttons">
			<button type="button" onclick={onclose}>Cancel</button>
			<button type="submit" class="primary" disabled={busy}>Create</button>
		</div>
	</form>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: #0008;
		display: grid;
		place-items: center;
		z-index: 10;
	}
	.dialog {
		width: min(560px, 92vw);
		background: var(--panel);
		border: 1px solid var(--border);
		border-radius: 10px;
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	h3 {
		margin: 0 0 4px;
	}
	label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		color: var(--muted);
	}
	label input,
	label textarea {
		color: var(--text);
	}
	.buttons {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}
	.error {
		color: var(--fail);
		margin: 0;
	}
</style>
