<script lang="ts">
	import { untrack } from 'svelte';
	import CodeEditor from './CodeEditor.svelte';
	import FormView from './FormView.svelte';
	import Icon from './Icon.svelte';
	import VarChips from './VarChips.svelte';
	import { api } from './api';
	import type { AnnotationKind } from './jsonpath';
	import type { Diagnostic, RequestDoc, RequestForm } from './types';
	import type { VarInfo, VarOption, VarStatus } from './vars';

	interface Props {
		cid: string;
		doc: RequestDoc;
		/** Variables offered after `$` / `${`. */
		vars: VarOption[];
		/** Status of a referenced variable in the selected environment. */
		varStatus: (name: string) => VarStatus;
		/** Value and source of a variable, for hover tooltips. */
		varInfo: (name: string) => VarInfo;
		/** Selected environment of the collection. */
		env: string;
		/** Opens the quick "set this variable" popover. */
		onsetvar: (name: string, anchor: DOMRect) => void;
		onsave: (content: string) => Promise<RequestDoc>;
		onrun: () => void;
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
		varInfo,
		env,
		onsetvar,
		onsave,
		onrun,
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
	let formView = $state<ReturnType<typeof FormView>>();

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
			notice = `Can't show as form: ${parsed.request.raw_reason ?? 'unsupported syntax'}. Fix it to run this request.`;
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

	/** Appends `@expect <line>` / `@capture <line>` to the request, unsaved. */
	export function addAnnotation(kind: AnnotationKind, line: string) {
		if (mode === 'form' && form) {
			const list = kind === 'expect' ? form.expects : form.captures;
			if (!list.includes(line)) list.push(line);
			formView?.showTab(kind === 'expect' ? 'assertions' : 'captures');
		} else {
			const lines = content.split('\n');
			const annotation = `# @${kind} ${line}`;
			if (lines.includes(annotation)) return;
			let end = 0;
			while (end < lines.length && lines[end].trim().startsWith('#')) end++;
			lines.splice(end, 0, annotation);
			content = lines.join('\n');
		}
		dirty = true;
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
				disabled={running}
				title="Run (Ctrl+Enter)"
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
		<div class="notice">Can't show as form: {rawReason}. Fix it to run this request.</div>
	{/if}

	<div class="body">
		{#if mode === 'form' && form}
			<FormView
				bind:this={formView}
				bind:form
				onedit={() => (dirty = true)}
				{modeSwitch}
				{actions}
				{vars}
				{varStatus}
				{varInfo}
				{env}
				{onsetvar}
			/>
		{:else}
			<div class="raw-bar">
				{@render modeSwitch()}
				<VarChips text={content} {varStatus} {onsetvar} />
				<span class="spacer"></span>
				{@render actions()}
			</div>
			<div class="code">
				<CodeEditor
					bind:value={content}
					{vars}
					{varStatus}
					{varInfo}
					{env}
					{onsetvar}
					onchange={() => (dirty = true)}
				/>
			</div>
		{/if}
	</div>

	{#if diagnostics.length}
		<ul class="diagnostics">
			{#each diagnostics as d, i (i)}
				<li class={d.severity}>
					<span class="pos">{d.line ? `line ${d.line}${d.column ? `:${d.column}` : ''}` : 'file'}:</span>
					{d.message}{#if d.label}<span class="label">: {d.label}</span>{/if}
					{#if d.help}<div class="hint">help: {d.help}</div>{/if}
					{#if d.note}<div class="hint">note: {d.note}</div>{/if}
				</li>
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
	.diagnostics .pos {
		opacity: 0.8;
	}
	.diagnostics .label {
		opacity: 0.85;
	}
	.diagnostics .hint {
		padding-left: 2ch;
		color: var(--muted);
	}
</style>
