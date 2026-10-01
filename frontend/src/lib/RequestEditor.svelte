<script lang="ts">
	import { untrack } from 'svelte';
	import CodeEditor from './CodeEditor.svelte';
	import FormView from './FormView.svelte';
	import { api } from './api';
	import type { Diagnostic, RequestDoc, RequestForm } from './types';

	interface Props {
		doc: RequestDoc;
		vars: string[];
		canRun: boolean;
		running: boolean;
		onsave: (content: string) => Promise<RequestDoc>;
		onrun: () => void;
		ondelete: () => void;
	}

	let { doc, vars, canRun, running, onsave, onrun, ondelete }: Props = $props();

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
		const parsed = await api.parse(content, doc.path);
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
		<span class="path mono" title={doc.path}>{doc.path}{dirty ? ' •' : ''}</span>
		<span class="spacer"></span>
		<button onclick={copyCurl} title="Copy the curl command">{copied ? 'Copied' : 'Copy curl'}</button>
		<button onclick={ondelete} title="Delete file">Delete</button>
		<button onclick={save} disabled={saving || !dirty} title="Save (Ctrl+S)">Save</button>
		<button class="primary" onclick={saveAndRun} disabled={!canRun || running} title="Run (Ctrl+Enter)">
			{running ? 'Running…' : 'Run'}
		</button>
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
		padding: 6px 10px;
		border-bottom: 1px solid var(--border);
		background: var(--panel);
	}
	.tabs {
		display: flex;
	}
	.tabs button {
		border-radius: 0;
	}
	.tabs button:first-child {
		border-radius: 6px 0 0 6px;
	}
	.tabs button:last-child {
		border-radius: 0 6px 6px 0;
		border-left: none;
	}
	.tabs .active {
		background: var(--accent-strong);
		color: #fff;
	}
	.path {
		color: var(--muted);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.spacer {
		flex: 1;
	}
	.notice {
		padding: 6px 12px;
		background: #3a2f12;
		color: #f8e3a8;
		font-size: 12px;
	}
	.notice.muted {
		background: var(--panel-2);
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
