<script lang="ts">
	import { untrack } from 'svelte';
	import CodeEditor from './CodeEditor.svelte';
	import FormView from './FormView.svelte';
	import Icon from './Icon.svelte';
	import Menu, { type MenuItem } from './Menu.svelte';
	import { api } from './api';
	import type { Diagnostic, RequestDoc, RequestForm } from './types';

	interface Props {
		cid: string;
		collection: string;
		doc: RequestDoc;
		vars: string[];
		canRun: boolean;
		running: boolean;
		stopping: boolean;
		onsave: (content: string) => Promise<RequestDoc>;
		onrun: () => void;
		/** Present while a run can be stopped. */
		onstop?: () => void;
		ondelete: () => void;
		/** Absent when there is no other collection to copy to. */
		oncopy?: () => void;
	}

	let { cid, collection, doc, vars, canRun, running, stopping, onsave, onrun, onstop, ondelete, oncopy }: Props =
		$props();

	// The parent remounts this component (via {#key}) when another file opens,
	// so initial values are taken once from the prop.
	const initial = untrack(() => $state.snapshot(doc));
	let mode = $state<'form' | 'raw'>(initial.form ? 'form' : 'raw');
	let content = $state(initial.content);
	let form = $state<RequestForm | null>(initial.form);
	let diagnostics = $state<Diagnostic[]>(initial.request.diagnostics);
	let rawReason = $state<string | null>(initial.request.raw_reason);
	let dirty = $state(false);
	let saving = $state(false);
	let notice = $state<string | null>(null);
	let copied = $state(false);

	let menu = $derived<MenuItem[]>([
		{ label: 'Copy as curl', icon: 'terminal', onselect: copyCurl },
		{
			label: 'Copy to collection…',
			icon: 'copy',
			disabled: !oncopy,
			hint: oncopy ? undefined : 'no other collection',
			onselect: () => oncopy?.()
		},
		{ label: 'Delete file', icon: 'trash', danger: true, onselect: ondelete }
	]);

	async function currentContent(): Promise<string> {
		if (mode === 'form' && form) {
			content = (await api.render($state.snapshot(form))).content;
		}
		return content;
	}

	async function toRaw() {
		if (mode === 'raw') return;
		await currentContent();
		mode = 'raw';
		notice = null;
	}

	async function toForm() {
		if (mode === 'form') return;
		const parsed = await api.parse(cid, content, doc.path);
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

	async function copyCurl() {
		const text = await currentContent();
		const lines = text.split('\n');
		let start = 0;
		while (start < lines.length && lines[start].trim().startsWith('#')) start++;
		await navigator.clipboard.writeText(lines.slice(start).join('\n').trim());
		copied = true;
		setTimeout(() => (copied = false), 1200);
	}
</script>

<div class="editor">
	<div class="toolbar">
		<div class="tabs" role="tablist">
			<button role="tab" aria-selected={mode === 'form'} class={{ active: mode === 'form' }} onclick={toForm}>
				Form
			</button>
			<button role="tab" aria-selected={mode === 'raw'} class={{ active: mode === 'raw' }} onclick={toRaw}>
				Raw
			</button>
		</div>
		<span class="path mono" title="{collection}: {doc.path}">
			<span class="col">{collection}</span> / {doc.path}{dirty ? ' •' : ''}
		</span>
		<span class="spacer"></span>
		{#if copied}<span class="copied"><Icon name="check" size={12} />Copied</span>{/if}
		<button onclick={save} disabled={saving || !dirty} title="Save (Ctrl+S)">
			<Icon name="save" size={13} />Save
		</button>
		{#if running && onstop}
			<button class="stop" onclick={onstop} disabled={stopping} title="Stop the run (Esc)">
				<Icon name="stop" size={10} />{stopping ? 'Stopping…' : 'Stop'}
			</button>
		{:else}
			<button class="primary" onclick={saveAndRun} disabled={!canRun || running} title="Run (Ctrl+Enter)">
				<Icon name="play" size={11} />{running ? 'Running…' : 'Run'}
			</button>
		{/if}
		<Menu title="More actions for this request" items={menu} />
	</div>

	{#if notice}
		<div class="notice">{notice}</div>
	{:else if mode === 'raw' && rawReason}
		<div class="notice muted">Raw mode: {rawReason}. The file runs as-is.</div>
	{/if}

	<div class="body">
		{#if mode === 'form' && form}
			<FormView bind:form onedit={() => (dirty = true)} />
		{:else}
			<CodeEditor bind:value={content} {vars} onchange={() => (dirty = true)} />
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
	.toolbar {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 44px;
		padding: 0 10px;
		border-bottom: 1px solid var(--border);
		background: var(--panel);
	}
	.toolbar button {
		flex: none;
		white-space: nowrap;
	}
	.tabs {
		display: flex;
		padding: 2px;
		gap: 2px;
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}
	.tabs button {
		min-height: 22px;
		padding: 0 10px;
		font-size: 12px;
		background: none;
		border-color: transparent;
		color: var(--muted);
	}
	.tabs button:hover:not(:disabled) {
		background: var(--panel-2);
		border-color: transparent;
		color: var(--text);
	}
	.tabs .active,
	.tabs .active:hover:not(:disabled) {
		background: var(--panel-3);
		color: var(--text);
	}
	.stop {
		color: var(--fail);
		border-color: var(--fail);
		background: var(--fail-bg);
	}
	.copied {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--ok);
		font-size: 12px;
	}
	.path {
		color: var(--muted);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.path .col {
		color: var(--text);
	}
	.spacer {
		flex: 1;
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
