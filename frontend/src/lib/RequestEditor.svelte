<script lang="ts">
	import { untrack } from 'svelte';
	import CodeEditor from './CodeEditor.svelte';
	import FormView from './FormView.svelte';
	import Icon from './Icon.svelte';
	import VarChips from './VarChips.svelte';
	import { api } from './api';
	import type { Diagnostic, RequestDoc, RequestForm } from './types';
	import type { VarOption, VarStatus } from './vars';

	interface Props {
		cid: string;
		doc: RequestDoc;
		/** Variables offered after `$` / `${`. */
		vars: VarOption[];
		/** Status of a referenced variable in the selected environment. */
		varStatus: (name: string) => VarStatus;
		/** Opens the quick "set this variable" popover. */
		onsetvar: (name: string, anchor: DOMRect) => void;
		onsave: (content: string) => Promise<RequestDoc>;
		onrun: () => void;
		/** False until the collection is trusted. */
		canRun: boolean;
		/** Some run is in progress (this request or another). */
		running: boolean;
		stopping: boolean;
		/** Present while a cancellable run is in progress. */
		onstop?: () => void;
		/** Unsaved edits, for the breadcrumb dot. */
		dirty?: boolean;
	}

	let {
		cid,
		doc,
		vars,
		varStatus,
		onsetvar,
		onsave,
		onrun,
		canRun,
		running,
		stopping,
		onstop,
		dirty = $bindable(false)
	}: Props = $props();

	let saving = $state(false);

	// The parent remounts this component (via {#key}) when another file opens,
	// so initial values are taken once from the prop.
	const initial = untrack(() => $state.snapshot(doc));
	let mode = $state<'form' | 'raw'>(initial.form ? 'form' : 'raw');
	let content = $state(initial.content);
	let form = $state<RequestForm | null>(initial.form);
	let diagnostics = $state<Diagnostic[]>(initial.request.diagnostics);
	let rawReason = $state<string | null>(initial.request.raw_reason);
	let notice = $state<string | null>(null);

	async function currentContent(): Promise<string> {
		if (mode === 'form' && form) {
			content = (await api.render($state.snapshot(form))).content;
		}
		return content;
	}

	async function toRaw() {
		if (mode === 'raw') return;
		try {
			await currentContent();
		} catch (e) {
			notice = `Can't switch to raw: ${e instanceof Error ? e.message : e}`;
			return;
		}
		mode = 'raw';
		notice = null;
	}

	async function toForm() {
		if (mode === 'form') return;
		let parsed;
		try {
			parsed = await api.parse(cid, content, doc.path);
		} catch (e) {
			notice = `Can't switch to form: ${e instanceof Error ? e.message : e}`;
			return;
		}
		diagnostics = parsed.request.diagnostics;
		rawReason = parsed.request.raw_reason;
		if (parsed.form) {
			form = parsed.form;
			mode = 'form';
			notice = null;
		} else {
			notice = `Can't show as form: ${parsed.request.raw_reason ?? 'unsupported syntax'}. The file still runs as-is.`;
		}
	}

	export async function save() {
		saving = true;
		try {
			const text = await currentContent();
			const saved = await onsave(text);
			diagnostics = saved.request.diagnostics;
			rawReason = saved.request.raw_reason;
			dirty = false;
		} finally {
			saving = false;
		}
	}

	export async function saveAndRun() {
		if (dirty) await save();
		onrun();
	}

	/** Copies the curl command without the annotation header. */
	export async function copyCurl() {
		const text = await currentContent();
		const lines = text.split('\n');
		let start = 0;
		while (start < lines.length && lines[start].trim().startsWith('#')) start++;
		await navigator.clipboard.writeText(lines.slice(start).join('\n').trim());
	}
</script>

{#snippet modeSwitch()}
	<div class="mode" role="tablist" aria-label="Editor mode">
		<button role="tab" aria-selected={mode === 'form'} class={{ active: mode === 'form' }} onclick={toForm}>
			Form
		</button>
		<button role="tab" aria-selected={mode === 'raw'} class={{ active: mode === 'raw' }} onclick={toRaw}>
			Raw
		</button>
	</div>
{/snippet}

{#snippet actions()}
	<div class="actions">
		<button
			class="icon-btn save"
			onclick={save}
			disabled={saving || !dirty}
			title={dirty ? 'Save (Ctrl+S)' : 'No unsaved changes'}
			aria-label="Save"
		>
			<Icon name="save" size={14} />
		</button>
		{#if running && onstop}
			<button class="run stop" onclick={onstop} disabled={stopping} title="Stop the run (Esc)">
				<Icon name="stop" size={10} />{stopping ? 'Stopping…' : 'Stop'}
			</button>
		{:else}
			<button
				class="run primary"
				onclick={saveAndRun}
				disabled={!canRun || running}
				title={canRun ? 'Run (Ctrl+Enter)' : 'Trust this collection to run requests'}
			>
				<Icon name="play" size={11} />{running ? 'Running…' : 'Run'}
			</button>
		{/if}
	</div>
{/snippet}

<div class="editor">
	{#if notice}
		<div class="notice">{notice}</div>
	{:else if mode === 'raw' && rawReason}
		<div class="notice muted">Raw mode: {rawReason}. The file runs as-is.</div>
	{/if}

	<div class="body">
		{#if mode === 'form' && form}
			<FormView bind:form onedit={() => (dirty = true)} {modeSwitch} {actions} {vars} {varStatus} {onsetvar} />
		{:else}
			<div class="raw-bar">
				{@render modeSwitch()}
				<VarChips text={content} {varStatus} {onsetvar} />
				<span class="spacer"></span>
				{@render actions()}
			</div>
			<div class="code">
				<CodeEditor bind:value={content} {vars} {varStatus} {onsetvar} onchange={() => (dirty = true)} />
			</div>
		{/if}
	</div>

	{#if diagnostics.length}
		<ul class="diagnostics">
			{#each diagnostics as d (d.line + d.message)}
				<li class={d.severity}>line {d.line}: {d.message}</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.editor {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}
	.mode {
		display: flex;
		flex: none;
		padding: 2px;
		gap: 2px;
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.mode button {
		min-height: 20px;
		padding: 0 9px;
		font-size: 11px;
		background: none;
		border-color: transparent;
		color: var(--muted);
	}
	.mode button:hover:not(:disabled) {
		background: var(--panel-2);
		border-color: transparent;
		color: var(--text);
	}
	.mode .active,
	.mode .active:hover:not(:disabled) {
		background: var(--panel-3);
		color: var(--text);
	}
	.raw-bar {
		display: flex;
		align-items: center;
		gap: 8px;
		min-height: 38px;
		padding: 4px 10px;
		border-bottom: 1px solid var(--border);
		background: var(--panel);
	}
	.spacer {
		flex: 1;
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 4px;
		flex: none;
	}
	.actions button {
		min-height: 32px;
		white-space: nowrap;
	}
	.save {
		width: 32px;
		padding: 0;
	}
	.save:not(:disabled) {
		color: var(--text);
	}
	.run {
		min-width: 76px;
		justify-content: center;
	}
	.stop {
		color: var(--fail);
		border-color: var(--fail);
		background: var(--fail-bg);
	}
	.notice {
		padding: 6px 12px;
		background: var(--warn-bg);
		border-bottom: 1px solid var(--warn-border);
		color: #f8e3a8;
		font-size: 12px;
	}
	.notice.muted {
		background: var(--panel-2);
		border-bottom-color: var(--border);
		color: var(--muted);
	}
	.body {
		flex: 1;
		min-height: 0;
		display: flex;
		flex-direction: column;
	}
	.code {
		flex: 1;
		min-height: 0;
	}
	.diagnostics {
		margin: 0;
		padding: 6px 12px;
		list-style: none;
		border-top: 1px solid var(--border);
		max-height: 110px;
		overflow: auto;
		font: 12px var(--mono);
	}
	.diagnostics .error {
		color: var(--fail);
	}
	.diagnostics .warning {
		color: var(--warn);
	}
</style>
