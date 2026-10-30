<script lang="ts">
	import { tick } from 'svelte';
	import CodeEditor from './CodeEditor.svelte';
	import FilePicker from './FilePicker.svelte';
	import Icon from './Icon.svelte';
	import {
		bodyMode,
		formRows,
		hasDataFlags,
		setFormRows,
		setUrlRows,
		urlRows,
		withBodyFlags,
		type BodyMode,
		type FormRow,
		type UrlRow
	} from './bodyForm';
	import { formatJson } from './format';
	import { varHover, varSuggest } from './suggest';
	import type { VarInfo, VarOption, VarStatus } from './vars';
	import type { RequestForm } from './types';

	interface Props {
		form: RequestForm;
		cid: string;
		onedit: () => void;
		vars: VarOption[];
		varStatus: (name: string) => VarStatus;
		varInfo: (name: string) => VarInfo;
		env: string;
		onsetvar: (name: string, anchor: DOMRect) => void;
	}

	let { form = $bindable(), cid, onedit, vars, varStatus, varInfo, env, onsetvar }: Props = $props();

	const suggest = varSuggest(() => vars);
	const hover = varHover(() => ({ info: varInfo, env, onedit: onsetvar }));

	const MODES: { mode: BodyMode; label: string }[] = [
		{ mode: 'none', label: 'None' },
		{ mode: 'raw', label: 'Raw' },
		{ mode: 'form', label: 'Form data' },
		{ mode: 'urlencoded', label: 'URL-encoded' },
		{ mode: 'file', label: 'Binary file' }
	];
	const RAW_FLAGS: { flag: string | null; label: string; title: string }[] = [
		{ flag: null, label: '-d', title: 'Sent as is; a leading @ reads a file without its newlines' },
		{ flag: '--data-raw', label: '--data-raw', title: 'Sent exactly as written; @ is not special' },
		{ flag: '--data-binary', label: '--data-binary', title: 'Sent as is; a leading @ reads a file unchanged' },
		{ flag: '--json', label: '--json', title: 'Also sets Content-Type and Accept to application/json' }
	];

	let detected = $derived(bodyMode(form.curl));
	/** A mode picked while the body is still empty; reset when another request loads. */
	let chosen = $derived.by<BodyMode | null>(() => {
		void form.curl;
		return null;
	});
	let mode = $derived(detected !== 'none' ? detected : (chosen ?? 'none'));
	let fRows = $derived(formRows(form.curl.flags));
	let uRows = $derived(urlRows(form.curl.flags));
	let filePath = $derived(detected === 'file' ? (form.curl.body ?? '').slice(1) : '');
	let extraData = $derived(hasDataFlags(form.curl.flags));
	let contentType = $derived(form.curl.headers.findIndex((h) => h.name.trim().toLowerCase() === 'content-type'));

	let rows = $state<HTMLElement>();
	let editor = $state<ReturnType<typeof CodeEditor>>();
	let formatError = $state<string | null>(null);
	let picking = $state.raw<{ current: string; apply: (path: string) => void } | null>(null);

	function edit() {
		onedit();
	}

	function isEmpty() {
		if (detected === 'none') return true;
		if (detected === 'raw') return !form.curl.body?.trim();
		return false;
	}

	function setMode(next: BodyMode) {
		if (next === mode) return;
		const label = MODES.find((m) => m.mode === next)!.label;
		if (!isEmpty() && !confirm(`Switch the body to ${label}? The current body will be removed.`)) return;
		form.curl.flags = withBodyFlags(form.curl.flags, []);
		form.curl.body = null;
		form.curl.body_flag = null;
		chosen = next;
		edit();
	}

	function setBody(text: string) {
		form.curl.body = text === '' ? null : text;
		edit();
	}

	function setRawFlag(flag: string | null) {
		form.curl.body_flag = flag;
		edit();
	}

	function formatBody() {
		try {
			const pretty = formatJson(form.curl.body ?? '');
			if (pretty !== form.curl.body) setBody(pretty);
		} catch (e) {
			formatError = e instanceof Error ? e.message : String(e);
		}
	}

	function setFile(path: string) {
		form.curl.body_flag = path ? '--data-binary' : null;
		form.curl.body = path ? `@${path}` : null;
		chosen = 'file';
		edit();
	}

	function writeForm(next: FormRow[]) {
		form.curl.flags = setFormRows(form.curl.flags, next);
		edit();
	}

	function writeUrl(next: UrlRow[]) {
		form.curl.flags = setUrlRows(form.curl.flags, next);
		edit();
	}

	function patchForm(i: number, patch: Partial<FormRow>) {
		writeForm(fRows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
	}

	function patchUrl(i: number, patch: Partial<UrlRow>) {
		writeUrl(uRows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
	}

	function blankFormRow(patch: Partial<FormRow>): FormRow {
		return { name: '', value: '', kind: 'text', type: '', flag: '--form-string', ...patch };
	}

	/** Typing into the trailing blank row adds a real row and keeps the caret there. */
	async function grow(add: (text: string) => number, field: string, e: Event & { currentTarget: HTMLInputElement }) {
		const text = e.currentTarget.value;
		e.currentTarget.value = '';
		const i = add(text);
		await tick();
		const next = rows?.querySelector<HTMLInputElement>(`[data-row="${i}"][data-field="${field}"]`);
		if (next) {
			next.focus();
			next.setSelectionRange(text.length, text.length);
		}
	}

	/** Drops a row left completely empty once focus leaves it. */
	function prune(row: { name: string; value: string; raw?: unknown }, remove: () => void, e: FocusEvent) {
		const el = e.currentTarget as HTMLElement;
		if (el.contains(e.relatedTarget as Node)) return;
		if (!row.raw && !row.name && !row.value) remove();
	}

	function removeContentType() {
		form.curl.headers.splice(contentType, 1);
		edit();
	}
</script>

{#snippet chooseButton(current: string, apply: (path: string) => void)}
	<button
		type="button"
		class="choose"
		title="Pick a file in the collection or upload one"
		onclick={() => (picking = { current, apply })}
	>
		<Icon name="folder" size={12} />Choose
	</button>
{/snippet}

<div class="body-editor">
	<div class="modes" role="radiogroup" aria-label="Body type">
		{#each MODES as m (m.mode)}
			<button
				type="button"
				role="radio"
				aria-checked={mode === m.mode}
				class={{ active: mode === m.mode }}
				onclick={() => setMode(m.mode)}>{m.label}</button
			>
		{/each}
	</div>

	{#if extraData}
		<p class="hint">This request also sets body data with curl flags under Settings.</p>
	{/if}

	{#if mode === 'none'}
		<p class="hint">No body is sent.</p>
	{:else if mode === 'raw'}
		<div class="raw-bar">
			<label>
				<span class="hint">Send with</span>
				<select
					class="mono"
					value={form.curl.body_flag ?? ''}
					onchange={(e) => setRawFlag(e.currentTarget.value || null)}
					title={RAW_FLAGS.find((f) => f.flag === form.curl.body_flag)?.title}
				>
					{#each RAW_FLAGS as f (f.label)}
						<option value={f.flag ?? ''}>{f.label}</option>
					{/each}
					{#if form.curl.body_flag && !RAW_FLAGS.some((f) => f.flag === form.curl.body_flag)}
						<option value={form.curl.body_flag}>{form.curl.body_flag}</option>
					{/if}
				</select>
			</label>
			<span class="spacer"></span>
			{#if form.curl.body?.trim()}
				<button type="button" class="tool" onclick={() => editor?.foldAll()} title="Fold all">
					<Icon name="collapse" size={12} />Fold
				</button>
				<button type="button" class="tool" onclick={() => editor?.unfoldAll()} title="Unfold all">
					<Icon name="expand" size={12} />Unfold
				</button>
				<button
					type="button"
					class={['tool', formatError && 'bad']}
					onclick={formatBody}
					title={formatError ?? 'Format JSON'}
					aria-label="Format JSON"
				>
					<Icon name={formatError ? 'alert' : 'braces'} size={12} />{formatError ? 'Not JSON' : 'Format'}
				</button>
			{/if}
		</div>
		<div class="code">
			<CodeEditor
				bind:this={editor}
				language="json"
				placeholder={'{"name": "test"}'}
				value={form.curl.body ?? ''}
				onchange={(v) => {
					formatError = null;
					setBody(v);
				}}
				{vars}
				{varStatus}
				{varInfo}
				{env}
				{onsetvar}
			/>
		</div>
	{:else if mode === 'form'}
		<div class="kv form-kv" bind:this={rows}>
			{#each fRows as row, i (i)}
				<div class="kv-row" onfocusout={(e) => prune(row, () => writeForm(fRows.filter((_, j) => j !== i)), e)}>
					{#if row.raw}
						<span class="prefix mono">{row.raw.flag}</span>
						<input
							class="mono wide"
							value={row.raw.arg}
							readonly
							title="Uses curl options this table can't edit; change it in the file's Raw view"
							aria-label="Form field"
						/>
					{:else}
						<select
							value={row.kind}
							onchange={(e) => patchForm(i, { kind: e.currentTarget.value as FormRow['kind'] })}
							aria-label="Field type"
						>
							<option value="text">Text</option>
							<option value="file">File</option>
						</select>
						<input
							class="mono"
							data-row={i}
							data-field="name"
							value={row.name}
							oninput={(e) => patchForm(i, { name: e.currentTarget.value })}
							placeholder="Name"
							aria-label="Field name"
						/>
						{#if row.kind === 'file'}
							<div class="file-cell">
								<input
									class="mono"
									data-row={i}
									data-field="value"
									value={row.value}
									oninput={(e) => patchForm(i, { value: e.currentTarget.value })}
									placeholder="files/avatar.png"
									aria-label="File path"
								/>
								<input
									class="mono type"
									value={row.type}
									oninput={(e) => patchForm(i, { type: e.currentTarget.value })}
									placeholder="type: auto"
									title="Content type of this part; guessed from the file name when empty"
									aria-label="Content type"
								/>
								{@render chooseButton(row.value, (p) => patchForm(i, { value: p, type: '' }))}
							</div>
						{:else}
							<input
								class="mono"
								data-row={i}
								data-field="value"
								value={row.value}
								oninput={(e) => patchForm(i, { value: e.currentTarget.value })}
								placeholder="Value"
								aria-label="Field value"
								{@attach suggest}
								{@attach hover}
							/>
						{/if}
					{/if}
					<button
						type="button"
						class="icon-btn del"
						title="Remove"
						aria-label="Remove"
						onclick={() => writeForm(fRows.filter((_, j) => j !== i))}
					>
						<Icon name="x" size={12} />
					</button>
				</div>
			{/each}
			<div class="kv-row blank">
				<span class="prefix mono">+</span>
				<input
					class="mono"
					placeholder="Add field"
					aria-label="New field name"
					oninput={(e) =>
						grow(
							(t) => (writeForm([...fRows, blankFormRow({ name: t })]), fRows.length - 1),
							'name',
							e
						)}
				/>
				<input
					class="mono"
					placeholder="Value"
					aria-label="New field value"
					oninput={(e) =>
						grow(
							(t) => (writeForm([...fRows, blankFormRow({ value: t })]), fRows.length - 1),
							'value',
							e
						)}
				/>
				<span></span>
			</div>
		</div>
		<p class="hint">
			Sent as <code>multipart/form-data</code>. File paths are relative to the collection folder.
		</p>
	{:else if mode === 'urlencoded'}
		<div class="kv url-kv" bind:this={rows}>
			{#each uRows as row, i (i)}
				<div class="kv-row" onfocusout={(e) => prune(row, () => writeUrl(uRows.filter((_, j) => j !== i)), e)}>
					{#if row.raw}
						<span class="prefix mono">{row.raw.flag}</span>
						<input
							class="mono wide"
							value={row.raw.arg}
							readonly
							title="Not a name=value field; change it in the file's Raw view"
							aria-label="Field"
						/>
					{:else}
						<input
							class="mono"
							data-row={i}
							data-field="name"
							value={row.name}
							oninput={(e) => patchUrl(i, { name: e.currentTarget.value })}
							placeholder="Name"
							aria-label="Field name"
						/>
						<input
							class="mono"
							data-row={i}
							data-field="value"
							value={row.value}
							oninput={(e) => patchUrl(i, { value: e.currentTarget.value })}
							placeholder="Value"
							aria-label="Field value"
							{@attach suggest}
							{@attach hover}
						/>
					{/if}
					<button
						type="button"
						class="icon-btn del"
						title="Remove"
						aria-label="Remove"
						onclick={() => writeUrl(uRows.filter((_, j) => j !== i))}
					>
						<Icon name="x" size={12} />
					</button>
				</div>
			{/each}
			<div class="kv-row blank">
				<input
					class="mono"
					placeholder="Add field"
					aria-label="New field name"
					oninput={(e) =>
						grow((t) => (writeUrl([...uRows, { name: t, value: '' }]), uRows.length - 1), 'name', e)}
				/>
				<input
					class="mono"
					placeholder="Value"
					aria-label="New field value"
					oninput={(e) =>
						grow((t) => (writeUrl([...uRows, { name: '', value: t }]), uRows.length - 1), 'value', e)}
				/>
				<span></span>
			</div>
		</div>
		<p class="hint">
			Sent as <code>application/x-www-form-urlencoded</code>; values are percent-encoded for you.
		</p>
	{:else}
		<div class="file-row">
			<input
				class="mono"
				value={filePath}
				oninput={(e) => setFile(e.currentTarget.value)}
				placeholder="files/payload.bin"
				aria-label="File path"
			/>
			{@render chooseButton(filePath, setFile)}
		</div>
		<p class="hint">
			The file is sent unchanged with <code>--data-binary</code>. Set a <code>Content-Type</code> header, or curl
			sends <code>application/x-www-form-urlencoded</code>.
		</p>
	{/if}

	{#if mode === 'form' && contentType !== -1}
		<p class="warn">
			<Icon name="alert" size={12} />
			A <code>Content-Type</code> header replaces the multipart boundary curl adds, so the server can't read the fields.
			<button type="button" onclick={removeContentType}>Remove header</button>
		</p>
	{/if}
</div>

{#if picking}
	<FilePicker
		{cid}
		current={picking.current}
		onpick={(p) => {
			picking?.apply(p);
			picking = null;
		}}
		onclose={() => (picking = null)}
	/>
{/if}

<style>
	.body-editor {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 8px;
		min-height: 0;
	}
	.modes {
		display: flex;
		align-self: flex-start;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		overflow: hidden;
	}
	.modes button {
		border: none;
		border-radius: 0;
		padding: 3px 10px;
		min-height: 26px;
		font-size: 12px;
		background: none;
		color: var(--muted);
	}
	.modes button + button {
		border-left: 1px solid var(--border);
	}
	.modes button.active {
		background: var(--panel-3);
		color: var(--text);
	}
	.raw-bar {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.raw-bar label {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.raw-bar select {
		min-height: 24px;
		font-size: 12px;
	}
	.spacer {
		flex: 1;
	}
	.tool {
		min-height: 22px;
		padding: 0 8px;
		gap: 4px;
		font-size: 11px;
		color: var(--muted);
		background: var(--panel-2);
	}
	.tool:hover:not(:disabled) {
		color: var(--text);
	}
	.tool.bad {
		color: var(--warn);
		border-color: var(--warn-border);
		background: var(--warn-bg);
	}
	.code {
		flex: 1;
		min-height: 160px;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		overflow: hidden;
	}
	.kv {
		display: flex;
		flex-direction: column;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		overflow: hidden;
	}
	.kv-row {
		display: grid;
		align-items: center;
		border-bottom: 1px solid var(--border);
	}
	.form-kv .kv-row {
		grid-template-columns: 72px minmax(0, 2fr) minmax(0, 3fr) 28px;
	}
	.url-kv .kv-row {
		grid-template-columns: minmax(0, 2fr) minmax(0, 3fr) 28px;
	}
	.url-kv .kv-row:has(.prefix) {
		grid-template-columns: auto minmax(0, 1fr) 28px;
	}
	.form-kv .kv-row:has(.wide) {
		grid-template-columns: auto minmax(0, 1fr) 28px;
	}
	.kv-row:last-child {
		border-bottom: none;
	}
	.kv-row input,
	.kv-row select {
		border: none;
		border-radius: 0;
		background: none;
		min-height: 30px;
		box-shadow: none;
	}
	.kv-row > * + input,
	.kv-row > * + .file-cell {
		border-left: 1px solid var(--border);
	}
	.kv-row select {
		font-size: 12px;
		color: var(--muted);
		cursor: pointer;
	}
	.kv-row input[readonly] {
		color: var(--muted);
	}
	.kv-row:hover {
		background: var(--panel);
	}
	.kv-row:focus-within {
		background: var(--panel-2);
	}
	.kv-row.blank input::placeholder {
		color: #87a39a88;
	}
	.prefix {
		padding: 0 4px 0 10px;
		color: var(--muted);
	}
	.file-cell {
		display: flex;
		align-items: center;
		min-width: 0;
		padding-right: 4px;
	}
	.file-cell input {
		flex: 1;
		min-width: 0;
	}
	.file-cell .type {
		flex: 0 0 96px;
		border-left: 1px solid var(--border);
		font-size: 11px;
	}
	.choose {
		flex: none;
		min-height: 22px;
		padding: 0 8px;
		gap: 4px;
		font-size: 11px;
	}
	.file-row {
		display: flex;
		gap: 6px;
	}
	.file-row input {
		flex: 1;
	}
	.del {
		justify-self: center;
		opacity: 0;
	}
	.kv-row:hover .del,
	.kv-row:focus-within .del {
		opacity: 1;
	}
	.hint {
		color: var(--muted);
		font-size: 11px;
		margin: 0;
	}
	.warn {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 6px;
		margin: 0;
		padding: 6px 10px;
		font-size: 12px;
		color: var(--warn);
		border: 1px solid var(--warn-border);
		background: var(--warn-bg);
		border-radius: var(--radius-sm);
	}
	.warn button {
		min-height: 22px;
		padding: 0 8px;
		font-size: 11px;
	}
</style>
