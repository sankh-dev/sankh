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
	/** The last import, shown for review when it left something out. */
	let imported = $state.raw<{ curl: string; name: string; content: string; notes: string[] } | null>(null);
	let notes = $derived(imported?.curl === curl && imported.name === name ? imported.notes : []);

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
			let content = TEMPLATE(name);
			if (curl.trim()) {
				if (notes.length) {
					content = imported!.content;
				} else {
					const result = await api.importCurl(curl, name);
					if (result.notes.length) {
						imported = { curl, name, ...result };
						return;
					}
					content = result.content;
				}
			}
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
		{#if notes.length}
			<div class="notes" role="status">
				<p>Left out of the imported request:</p>
				<ul>
					{#each notes as note, i (i)}
						<li>{note}</li>
					{/each}
				</ul>
			</div>
		{/if}
		{#if error}<p class="error">{error}</p>{/if}
		<div class="buttons">
			<button type="button" onclick={onclose}>Cancel</button>
			<button type="submit" class="primary" disabled={busy}>{notes.length ? 'Create anyway' : 'Create'}</button>
		</div>
	</form>
</div>

<style>
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
	.error {
		color: var(--fail);
		margin: 0;
	}
	.notes {
		padding: 6px 10px;
		background: var(--warn-bg);
		border: 1px solid var(--warn-border);
		border-radius: var(--radius-sm);
		color: #f8e3a8;
		font-size: 12px;
	}
	.notes p {
		margin: 0 0 2px;
	}
	.notes ul {
		margin: 0;
		padding-left: 18px;
	}
</style>
