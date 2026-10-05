<script module lang="ts">
	export type EnvChange = { type: 'renamed'; from: string; to: string } | { type: 'deleted'; name: string } | null;
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from './api';
	import type { EnvFileVar } from './types';

	interface Props {
		cid: string;
		collection: string;
		/** Environment selected for this collection, opened first. */
		current: string;
		onchanged: (change: EnvChange) => Promise<void>;
		onclose: () => void;
	}

	let { cid, collection, current, onchanged, onclose }: Props = $props();

	/** Not a valid environment name, so it cannot clash with one. */
	const LOCAL = ' local';
	const ENV_NAME = /^[a-z0-9][a-z0-9._-]*$/;
	const VAR_NAME = /^[A-Za-z_][A-Za-z0-9_]*$/;
	const SECRET = /TOKEN|SECRET|KEY|PASSWORD|PASSWD|AUTH|COOKIE/i;

	type Row = { id: number; name: string; value: string; reveal: boolean };

	let envs = $state.raw<string[]>([]);
	let defaultEnv = $state<string | null>(null);
	let target = $state(LOCAL);
	let rows = $state<Row[]>([]);
	let original = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);
	let nextId = 0;

	let isLocal = $derived(target === LOCAL);
	let snapshot = $derived(JSON.stringify(cleaned(rows)));
	let dirty = $derived(snapshot !== original);
	let problems = $derived.by(() => {
		const out: Record<number, string> = {};
		const seen: string[] = [];
		for (const r of rows) {
			const name = r.name.trim();
			if (!name && !r.value) continue;
			if (!VAR_NAME.test(name)) out[r.id] = 'Use letters, digits and _; do not start with a digit';
			else if (seen.includes(name)) out[r.id] = 'Listed twice';
			seen.push(name);
		}
		return out;
	});
	let invalid = $derived(Object.keys(problems).length > 0);

	function message(e: unknown) {
		return e instanceof Error ? e.message : String(e);
	}

	function cleaned(list: Row[]): EnvFileVar[] {
		return list
			.filter((r) => r.name.trim() || r.value)
			.map((r) => ({ name: r.name.trim(), value: r.value }));
	}

	function setRows(vars: EnvFileVar[]) {
		rows = vars.map((v) => ({ id: nextId++, name: v.name, value: v.value, reveal: false }));
		original = JSON.stringify(cleaned(rows));
	}

	async function loadList() {
		const e = await api.envs(cid);
		envs = e.envs;
		defaultEnv = e.default;
	}

	async function loadTarget(next: string) {
		error = null;
		try {
			const res = next === LOCAL ? await api.envLocal(cid) : await api.envFile(cid, next);
			target = next;
			setRows(res.vars);
		} catch (e) {
			error = message(e);
		}
	}

	function confirmDiscard() {
		return !dirty || confirm('Discard unsaved changes?');
	}

	async function pick(next: string) {
		if (next === target || !confirmDiscard()) return;
		await loadTarget(next);
	}

	function addRow() {
		rows.push({ id: nextId++, name: '', value: '', reveal: true });
	}

	function removeRow(id: number) {
		rows = rows.filter((r) => r.id !== id);
	}

	async function save() {
		if (invalid) return;
		busy = true;
		error = null;
		try {
			const vars = cleaned(rows);
			const res = isLocal ? await api.saveEnvLocal(cid, vars) : await api.saveEnvFile(cid, target, vars);
			setRows(res.vars);
			await onchanged(null);
		} catch (e) {
			error = message(e);
		} finally {
			busy = false;
		}
	}

	async function revert() {
		await loadTarget(target);
	}

	async function act(work: () => Promise<EnvChange>, next?: () => string) {
		busy = true;
		error = null;
		try {
			const change = await work();
			await loadList();
			await onchanged(change);
			if (next) await loadTarget(next());
		} catch (e) {
			error = message(e);
		} finally {
			busy = false;
		}
	}

	function askName(prompt: string, initial = '') {
		const name = window.prompt(prompt, initial)?.trim();
		if (!name) return null;
		if (!ENV_NAME.test(name)) {
			error = `"${name}" is not a valid name: use lowercase letters, digits, ".", "_" and "-"`;
			return null;
		}
		return name;
	}

	async function create() {
		if (!confirmDiscard()) return;
		const name = askName('New environment name');
		if (!name) return;
		const copyFrom = !isLocal && confirm(`Copy the variables of "${target}"?`) ? target : undefined;
		await act(
			async () => {
				await api.createEnv(cid, name, copyFrom);
				return null;
			},
			() => name
		);
	}

	async function rename() {
		if (isLocal || !confirmDiscard()) return;
		const from = target;
		const to = askName(`Rename "${from}" to`, from);
		if (!to || to === from) return;
		await act(
			async () => {
				await api.renameEnv(cid, from, to);
				return { type: 'renamed', from, to };
			},
			() => to
		);
	}

	async function remove() {
		if (isLocal) return;
		const name = target;
		if (!confirm(`Delete environment "${name}"?\n\nThis removes environments/${name}.env from disk.`)) return;
		await act(
			async () => {
				await api.deleteEnv(cid, name);
				return { type: 'deleted', name };
			},
			() => LOCAL
		);
	}

	async function makeDefault() {
		if (isLocal) return;
		const name = defaultEnv === target ? null : target;
		await act(async () => {
			await api.setDefaultEnv(cid, name);
			return null;
		});
	}

	function close() {
		if (confirmDiscard()) onclose();
	}

	onMount(async () => {
		try {
			await loadList();
		} catch (e) {
			error = message(e);
		}
		await loadTarget(current && envs.includes(current) ? current : LOCAL);
	});
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && close()} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && close()}>
	<div class="dialog" role="dialog" aria-label="Environments for {collection}">
		<h3>Environments <span class="muted">· {collection}</span></h3>
		<div class="body">
			<nav aria-label="Environment files">
				{#each envs as e (e)}
					<button class={['item', target === e && 'active']} onclick={() => pick(e)}>
						<span class="mono">{e}</span>
						{#if defaultEnv === e}<span class="badge">default</span>{/if}
					</button>
				{/each}
				<button class={['item', isLocal && 'active']} onclick={() => pick(LOCAL)}>
					<span class="mono">.env.local</span>
					<span class="muted small">local overrides</span>
				</button>
				<button class="new" onclick={create} disabled={busy}>+ New environment</button>
			</nav>

			<section class="vars">
				<div class="toolbar">
					<span class="mono path">{isLocal ? '.env.local' : `environments/${target}.env`}</span>
					{#if !isLocal}
						<button onclick={makeDefault} disabled={busy}>
							{defaultEnv === target ? 'Unset default' : 'Make default'}
						</button>
						<button onclick={rename} disabled={busy}>Rename</button>
						<button class="danger" onclick={remove} disabled={busy}>Delete</button>
					{/if}
				</div>
				<p class="muted small">
					{#if isLocal}
						Gitignored. Overrides every environment; keep secrets here. Values set in the process environment
						still win.
					{:else}
						Committed with the collection. Keep secrets in <code>.env.local</code> instead.
					{/if}
					<code>$&#123;VAR&#125;</code> expands; write <code>\$</code> for a literal dollar sign.
				</p>

				<div class="table">
					{#each rows as row (row.id)}
						{@const masked = SECRET.test(row.name) && !row.reveal}
						<div class="row">
							<input
								class={['mono', problems[row.id] && 'bad']}
								bind:value={row.name}
								placeholder="NAME"
								aria-label="Variable name"
								title={problems[row.id] ?? ''}
							/>
							<input
								class="mono"
								type={masked ? 'password' : 'text'}
								bind:value={row.value}
								placeholder="value"
								aria-label="Value of {row.name || 'new variable'}"
							/>
							{#if SECRET.test(row.name)}
								<button
									class="icon"
									onclick={() => (row.reveal = !row.reveal)}
									title={row.reveal ? 'Hide value' : 'Show value'}
								>
									{row.reveal ? 'hide' : 'show'}
								</button>
							{:else}
								<span class="icon-space"></span>
							{/if}
							<button class="icon" onclick={() => removeRow(row.id)} title="Remove variable">✕</button>
						</div>
						{#if problems[row.id]}<p class="problem">{problems[row.id]}</p>{/if}
					{:else}
						<p class="muted small">No variables yet.</p>
					{/each}
				</div>
				<button class="add" onclick={addRow}>+ Add variable</button>
			</section>
		</div>

		{#if error}<p class="error">{error}</p>{/if}
		<div class="buttons">
			{#if dirty}<span class="warn small">Unsaved changes</span>{/if}
			<button onclick={revert} disabled={busy || !dirty}>Revert</button>
			<button class="primary" onclick={save} disabled={busy || !dirty || invalid}>
				{busy ? 'Saving...' : 'Save'}
			</button>
			<button onclick={close}>Close</button>
		</div>
	</div>
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
		width: min(860px, 94vw);
		height: min(620px, 90vh);
		background: var(--panel);
		border: 1px solid var(--border);
		border-radius: 10px;
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	h3,
	p {
		margin: 0;
	}
	.body {
		flex: 1;
		min-height: 0;
		display: grid;
		grid-template-columns: 200px minmax(0, 1fr);
		gap: 12px;
	}
	nav {
		display: flex;
		flex-direction: column;
		gap: 2px;
		overflow: auto;
		border-right: 1px solid var(--border);
		padding-right: 8px;
	}
	.item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 6px;
		background: none;
		border-color: transparent;
		text-align: left;
	}
	.item.active {
		background: var(--bg);
		border-color: var(--border);
	}
	.new,
	.add {
		background: none;
		border-style: dashed;
		color: var(--muted);
		margin-top: 6px;
	}
	.add {
		align-self: flex-start;
	}
	.badge {
		font-size: 10px;
		padding: 0 5px;
		border-radius: 4px;
		border: 1px solid currentColor;
		color: var(--muted);
	}
	.vars {
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-height: 0;
	}
	.toolbar {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.path {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.table {
		flex: 1;
		overflow: auto;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.row {
		display: grid;
		grid-template-columns: minmax(0, 2fr) minmax(0, 3fr) 48px 28px;
		gap: 4px;
		align-items: center;
	}
	.icon {
		padding: 2px 4px;
		font-size: 11px;
		background: none;
	}
	.bad {
		border-color: var(--fail);
	}
	.problem {
		color: var(--fail);
		font-size: 11px;
	}
	.danger {
		color: var(--fail);
	}
	.muted {
		color: var(--muted);
	}
	.small {
		font-size: 12px;
	}
	.warn {
		color: var(--warn);
		margin-right: auto;
	}
	.error {
		color: var(--fail);
	}
	.buttons {
		display: flex;
		justify-content: flex-end;
		align-items: center;
		gap: 8px;
	}
</style>
