<script lang="ts">
	import { api } from './api';
	import type { EnvImportPlan } from './types';

	interface Props {
		cid: string;
		collection: string;
		onimported: () => Promise<void>;
		onclose: () => void;
	}

	let { cid, collection, onimported, onclose }: Props = $props();

	let texts = $state.raw<string[]>([]);
	let files = $state.raw<string[]>([]);
	let plan = $state.raw<EnvImportPlan | null>(null);
	let secretsToLocal = $state(false);
	let error = $state<string | null>(null);
	let busy = $state(false);

	let changes = $derived(
		plan ? plan.envs.some((e) => e.status === 'new' || e.added.length || e.updated.length) : false
	);
	let localChanges = $derived(plan ? plan.local_added.length + plan.local_updated.length : 0);

	function message(e: unknown) {
		return e instanceof Error ? e.message : String(e);
	}

	async function pick(event: Event) {
		const list = Array.from((event.currentTarget as HTMLInputElement).files ?? []);
		files = list.map((f) => f.name);
		texts = await Promise.all(list.map((f) => f.text()));
		plan = null;
		error = null;
		if (!texts.length) return;
		try {
			plan = await api.previewPostmanEnvs(cid, texts);
		} catch (e) {
			error = message(e);
		}
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!plan) return;
		error = null;
		busy = true;
		try {
			await api.importPostmanEnvs(cid, texts, secretsToLocal);
			await onimported();
		} catch (e) {
			error = message(e);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
	<form class="dialog" onsubmit={submit} aria-label="Import Postman environments into {collection}">
		<h3>Import Postman environments <span class="muted">· {collection}</span></h3>
		<p class="muted">
			Each environment goes to <code>environments/&lt;name&gt;.env</code>. An existing file is merged: matching
			variables take the Postman value, new ones are added, and the rest are kept. Nothing is written until you
			choose <strong>Import</strong>.
		</p>

		<label class="field">
			<span>Environment exports</span>
			<input type="file" accept=".json,application/json" multiple onchange={pick} />
			{#if files.length}<span class="muted mono">{files.join(', ')}</span>{/if}
		</label>

		{#if plan}
			<div class="summary">
				<table>
					<thead>
						<tr><th>Postman</th><th>File</th><th>Changes</th></tr>
					</thead>
					<tbody>
						{#each plan.envs as e (e.env)}
							<tr>
								<td>{e.source}</td>
								<td class="mono">
									{e.file}
									<span class={['tag', e.status]}>{e.status === 'new' ? 'new' : 'merge'}</span>
								</td>
								<td>
									{#if e.added.length}<span class="ok" title={e.added.join(', ')}>+{e.added.length} added</span>{/if}
									{#if e.updated.length}<span class="warn" title={e.updated.join(', ')}
											>~{e.updated.length} updated</span
										>{/if}
									{#if e.unchanged}<span class="muted">{e.unchanged} unchanged</span>{/if}
									{#if !e.added.length && !e.updated.length && !e.unchanged}<span class="muted">no variables</span>{/if}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
				{#if plan.renamed.length}
					<details>
						<summary>{plan.renamed.length} renamed variable(s)</summary>
						<ul class="mono">
							{#each plan.renamed as r (r.from)}<li>{r.from} &rarr; {r.to}</li>{/each}
						</ul>
					</details>
				{/if}
				{#if plan.secrets.length}
					<div class="secrets">
						<p>
							Secrets are never written to <code>environments/</code>:
							<span class="mono">{plan.secrets.join(', ')}</span>
						</p>
						<label class="check">
							<input type="checkbox" bind:checked={secretsToLocal} />
							Copy their values into <code>.env.local</code> (gitignored)
						</label>
						{#if secretsToLocal && plan.local_updated.length}
							<p class="warn">Replaces existing values: <span class="mono">{plan.local_updated.join(', ')}</span></p>
						{/if}
					</div>
				{/if}
			</div>
		{/if}

		{#if error}<p class="error">{error}</p>{/if}
		<div class="buttons">
			<button type="button" onclick={onclose}>Cancel</button>
			<button
				type="submit"
				class="primary"
				disabled={busy || !plan || (!changes && !(secretsToLocal && localChanges))}
			>
				{busy ? 'Importing...' : 'Import'}
			</button>
		</div>
	</form>
</div>

<style>
	.dialog {
		width: min(620px, 92vw);
	}
	p {
		margin: 0;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.summary {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 8px 10px;
		border: 1px solid var(--border);
		border-radius: 6px;
		background: var(--bg);
		max-height: 340px;
		overflow: auto;
	}
	table {
		border-collapse: collapse;
		width: 100%;
		font-size: 12px;
	}
	th {
		text-align: left;
		color: var(--muted);
		font-weight: 600;
		padding: 2px 6px 4px 0;
	}
	td {
		padding: 3px 6px 3px 0;
		vertical-align: top;
	}
	td span + span {
		margin-left: 8px;
	}
	.tag {
		font-size: 10px;
		margin-left: 6px;
		padding: 0 5px;
		border-radius: 4px;
		border: 1px solid currentColor;
		font-family: var(--sans);
	}
	.tag.new {
		color: var(--ok);
	}
	.tag.merge {
		color: var(--warn);
	}
	.secrets {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	ul {
		margin: 4px 0;
		padding-left: 20px;
	}
	.check {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.muted {
		color: var(--muted);
	}
	.ok {
		color: var(--ok);
	}
	.warn {
		color: var(--warn);
	}
	.error {
		color: var(--fail);
	}
</style>
