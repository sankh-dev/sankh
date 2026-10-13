<script lang="ts">
	import { tick, type Snippet } from 'svelte';
	import Icon from './Icon.svelte';
	import VarChips from './VarChips.svelte';
	import { formatJson } from './format';
	import { varSuggest } from './suggest';
	import { needsValue, refs, type VarOption, type VarStatus } from './vars';
	import type { RequestForm } from './types';

	interface Props {
		form: RequestForm;
		onedit: () => void;
		/** Form/Raw switch, shown before the title. */
		modeSwitch: Snippet;
		/** Save and Run buttons, shown after the URL. */
		actions: Snippet;
		/** Variables offered after `$` / `${`. */
		vars: VarOption[];
		varStatus: (name: string) => VarStatus;
		onsetvar: (name: string, anchor: DOMRect) => void;
	}

	let { form = $bindable(), onedit, modeSwitch, actions, vars, varStatus, onsetvar }: Props = $props();

	const suggest = varSuggest(() => vars);

	type Tab = 'headers' | 'body' | 'assertions' | 'captures' | 'settings';
	const TAB_KEY = 'sankh-form-tab';
	const TABS: Tab[] = ['headers', 'body', 'assertions', 'captures', 'settings'];
	const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];
	const TAG = /^[a-z0-9_-]+$/;

	const saved = localStorage.getItem(TAB_KEY) as Tab | null;
	let tab = $state<Tab>(saved && TABS.includes(saved) ? saved : 'headers');
	let addingTag = $state(false);
	let tagDraft = $state('');
	let tagBad = $state(false);
	let rows = $state<HTMLElement>();

	let flagsText = $derived(form.curl.flags.join(' '));
	let headerText = $derived(form.curl.headers.map((h) => `${h.name} ${h.value}`).join('\n'));
	let checkedText = $derived(`${form.curl.url}\n${headerText}\n${form.curl.body ?? ''}\n${flagsText}`);
	let counts = $derived<Partial<Record<Tab, number>>>({
		headers: form.curl.headers.filter((h) => h.name || h.value).length,
		assertions: form.expects.filter(Boolean).length,
		captures: form.captures.filter(Boolean).length
	});
	let warn = $derived<Partial<Record<Tab, boolean>>>({
		headers: flaggedIn(headerText),
		body: flaggedIn(form.curl.body ?? ''),
		settings: flaggedIn(flagsText)
	});

	$effect(() => localStorage.setItem(TAB_KEY, tab));

	const LABEL: Record<Tab, string> = {
		headers: 'Headers',
		body: 'Body',
		assertions: 'Assertions',
		captures: 'Captures',
		settings: 'Settings'
	};

	function flaggedIn(text: string) {
		return refs(text).some((n) => needsValue(varStatus(n)));
	}

	function edit() {
		onedit();
	}

	function setFlags(text: string) {
		form.curl.flags = text.split(/\s+/).filter(Boolean);
		edit();
	}

	function setBody(text: string) {
		form.curl.body = text === '' ? null : text;
		edit();
	}

	let formatError = $state<string | null>(null);

	function formatBody() {
		try {
			const pretty = formatJson(form.curl.body ?? '');
			if (pretty !== form.curl.body) setBody(pretty);
		} catch (e) {
			formatError = e instanceof Error ? e.message : String(e);
		}
	}

	function commitTag() {
		const tag = tagDraft.trim().toLowerCase();
		if (!tag) {
			addingTag = false;
			return true;
		}
		if (!TAG.test(tag)) {
			tagBad = true;
			return false;
		}
		if (!form.tags.includes(tag)) form.tags.push(tag);
		tagDraft = '';
		tagBad = false;
		edit();
		return true;
	}

	function tagKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' || e.key === ' ' || e.key === ',') {
			e.preventDefault();
			commitTag();
		} else if (e.key === 'Escape') {
			e.stopPropagation();
			tagDraft = '';
			tagBad = false;
			addingTag = false;
		} else if (e.key === 'Backspace' && !tagDraft && form.tags.length) {
			form.tags.pop();
			edit();
		}
	}

	function removeTag(i: number) {
		form.tags.splice(i, 1);
		edit();
	}

	function focusInput(node: HTMLInputElement) {
		node.focus();
	}

	function removeAt(list: unknown[], i: number) {
		list.splice(i, 1);
		edit();
	}

	/** Typing into the trailing blank row adds a real row and keeps the caret there. */
	async function grow(add: () => number, field: string, e: Event & { currentTarget: HTMLInputElement }) {
		const text = e.currentTarget.value;
		e.currentTarget.value = '';
		const i = add();
		edit();
		await tick();
		const next = rows?.querySelector<HTMLInputElement>(`[data-row="${i}"][data-field="${field}"]`);
		if (next) {
			next.focus();
			next.setSelectionRange(text.length, text.length);
		}
	}

	/** Drops a row left completely empty once focus leaves it. */
	function pruneHeader(i: number, e: FocusEvent) {
		const row = e.currentTarget as HTMLElement;
		if (row.contains(e.relatedTarget as Node)) return;
		const h = form.curl.headers[i];
		if (h && !h.name && !h.value) removeAt(form.curl.headers, i);
	}

	function pruneLine(list: string[], i: number, e: FocusEvent) {
		const row = e.currentTarget as HTMLElement;
		if (row.contains(e.relatedTarget as Node)) return;
		if (list[i] === '') removeAt(list, i);
	}
</script>

{#snippet lines(list: string[], prefix: string, placeholder: string, label: string)}
	<div class="kv single" bind:this={rows}>
		{#each list as _, i (i)}
			<div class="kv-row" onfocusout={(e) => pruneLine(list, i, e)}>
				<span class="prefix mono">{prefix}</span>
				<input
					class="mono"
					data-row={i}
					data-field="v"
					bind:value={list[i]}
					{placeholder}
					aria-label={label}
					{@attach suggest}
				/>
				<button class="icon-btn del" title="Remove" aria-label="Remove" onclick={() => removeAt(list, i)}>
					<Icon name="x" size={12} />
				</button>
			</div>
		{/each}
		<div class="kv-row blank">
			<span class="prefix mono">{prefix}</span>
			<input
				class="mono"
				{placeholder}
				aria-label="New {label.toLowerCase()}"
				oninput={(e) => grow(() => list.push(e.currentTarget.value) - 1, 'v', e)}
			/>
			<span></span>
		</div>
	</div>
{/snippet}

<div class="form" oninput={edit} onchange={edit}>
	<div class="top">
		<div class="title-row">
			{@render modeSwitch()}
			<input class="ghost-input title" bind:value={form.name} placeholder="Untitled request" aria-label="Request name" />
			<div
				class="tags"
				title="Labels for picking requests in CI, e.g. sankh run --tag smoke. Lowercase letters, digits, - and _."
			>
				{#each form.tags as t, i (t)}
					<span class="tag">
						#{t}
						<button class="tag-x" title="Remove tag {t}" aria-label="Remove tag {t}" onclick={() => removeTag(i)}>
							<Icon name="x" size={9} />
						</button>
					</span>
				{/each}
				{#if addingTag}
					<input
						class={['mono', 'tag-input', tagBad && 'bad']}
						bind:value={tagDraft}
						placeholder="tag"
						aria-label="New tag"
						onkeydown={tagKeydown}
						oninput={() => (tagBad = false)}
						onblur={() => commitTag() && (addingTag = false)}
						{@attach focusInput}
					/>
				{:else}
					<button class="add-tag" onclick={() => (addingTag = true)} title="Add a tag">
						<Icon name="plus" size={10} />{form.tags.length ? '' : 'tag'}
					</button>
				{/if}
			</div>
		</div>
		<textarea
			class="ghost-input desc"
			rows="1"
			value={form.description ?? ''}
			placeholder="Add a description"
			aria-label="Description"
			oninput={(e) => (form.description = e.currentTarget.value || null)}
		></textarea>

		<div class="url-row">
			<div class="urlbar">
				<select
					class="method method-pill method-{form.curl.method}"
					bind:value={form.curl.method}
					onchange={edit}
					aria-label="Method"
				>
					{#each METHODS as m (m)}
						<option value={m}>{m}</option>
					{/each}
				</select>
				<input
					class="mono url"
					bind:value={form.curl.url}
					placeholder="$BASE_URL/path"
					aria-label="URL"
					title="$VAR expands from the environment; \$ is a literal dollar sign"
					{@attach suggest}
				/>
			</div>
			{@render actions()}
		</div>
		<VarChips text={checkedText} {varStatus} {onsetvar} />
	</div>

	<div class="tabbar">
		<div class="tabs" role="tablist" aria-label="Request parts">
			{#each TABS as t (t)}
				<button
					role="tab"
					class={['tab', tab === t && 'active']}
					aria-selected={tab === t}
					onclick={() => (tab = t)}
				>
					{LABEL[t]}
					{#if counts[t]}<span class="count">{counts[t]}</span>{/if}
					{#if warn[t]}<span class="dot" title="References a variable without a value"></span>{/if}
				</button>
			{/each}
		</div>
	</div>

	<div class="panel" role="tabpanel" aria-label={LABEL[tab]}>
		{#if tab === 'headers'}
			<div class="kv" bind:this={rows}>
				{#each form.curl.headers as header, i (i)}
					<div class="kv-row" onfocusout={(e) => pruneHeader(i, e)}>
						<input
							class="mono"
							data-row={i}
							data-field="name"
							bind:value={header.name}
							placeholder="Name"
							aria-label="Header name"
						/>
						<input
							class="mono"
							data-row={i}
							data-field="value"
							bind:value={header.value}
							placeholder="Value"
							aria-label="Header value"
							{@attach suggest}
						/>
						<button
							class="icon-btn del"
							title="Remove"
							aria-label="Remove"
							onclick={() => removeAt(form.curl.headers, i)}
						>
							<Icon name="x" size={12} />
						</button>
					</div>
				{/each}
				<div class="kv-row blank">
					<input
						class="mono"
						placeholder="Add header"
						aria-label="New header name"
						oninput={(e) => grow(() => form.curl.headers.push({ name: e.currentTarget.value, value: '' }) - 1, 'name', e)}
					/>
					<input
						class="mono"
						placeholder="Value"
						aria-label="New header value"
						oninput={(e) => grow(() => form.curl.headers.push({ name: '', value: e.currentTarget.value }) - 1, 'value', e)}
					/>
					<span></span>
				</div>
			</div>
		{:else if tab === 'body'}
			<div class="body-wrap">
				<textarea
					class="mono body"
					value={form.curl.body ?? ''}
					placeholder={'{"name": "test"}'}
					oninput={(e) => {
						formatError = null;
						setBody(e.currentTarget.value);
					}}
					aria-label="Body"
					{@attach suggest}
				></textarea>
				{#if form.curl.body?.trim()}
					<button
						class={['format', formatError && 'bad']}
						onclick={formatBody}
						title={formatError ?? 'Format JSON'}
						aria-label="Format JSON"
					>
						<Icon name={formatError ? 'alert' : 'braces'} size={12} />{formatError ? 'Not JSON' : 'Format'}
					</button>
				{/if}
			</div>
		{:else if tab === 'assertions'}
			{@render lines(form.expects, '@expect', 'status 201  ·  json .data.id exists', 'Assertion')}
			{#if !counts.assertions}
				<p class="hint">No assertions: any 2xx status passes.</p>
			{/if}
		{:else if tab === 'captures'}
			{@render lines(form.captures, '@capture', 'TOKEN=.data.token  ·  ID=header X-Id', 'Capture')}
			<p class="hint">Captured values are available as <code>$NAME</code> to later requests in the run.</p>
		{:else}
			<div class="settings">
				<label for="f-timeout">Timeout</label>
				<div class="field">
					<input
						id="f-timeout"
						class="mono timeout"
						value={form.timeout ?? ''}
						placeholder="default"
						aria-describedby="f-timeout-hint"
						oninput={(e) => (form.timeout = e.currentTarget.value.trim() || null)}
					/>
					<p class="hint" id="f-timeout-hint">
						e.g. <code>500ms</code>, <code>10s</code>, <code>2m</code>. Empty uses <code>timeout</code> from
						<code>sankh.toml</code> (30s if unset).
					</p>
				</div>
				<label for="f-flags">curl flags</label>
				<div class="field">
					<input
						id="f-flags"
						class="mono"
						{@attach suggest}
						value={flagsText}
						placeholder="-sS --compressed"
						aria-describedby="f-flags-hint"
						onchange={(e) => setFlags(e.currentTarget.value)}
					/>
					<p class="hint" id="f-flags-hint">Extra flags passed to curl, space-separated.</p>
				</div>
			</div>
		{/if}
	</div>
</div>

<style>
	.form {
		height: 100%;
		display: flex;
		flex-direction: column;
		min-height: 0;
	}
	.top {
		padding: 12px 14px 10px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.title-row {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
	}
	.title {
		flex: 1 1 200px;
		font-size: 16px;
		font-weight: 600;
		padding: 2px 6px;
		margin-left: -6px;
	}
	.desc {
		field-sizing: content;
		min-height: 0;
		max-height: 10lh;
		resize: none;
		color: var(--muted);
		padding: 2px 6px;
		margin-left: -6px;
		font-family: inherit;
	}
	.desc:focus {
		color: var(--text);
	}
	.tags {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 4px;
	}
	.tag {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		padding: 0 2px 0 7px;
		height: 20px;
		border-radius: 999px;
		background: var(--panel-3);
		color: var(--muted);
		font: 11px var(--mono);
	}
	.tag-x {
		min-height: 16px;
		width: 16px;
		padding: 0;
		border: none;
		border-radius: 50%;
		background: none;
		color: inherit;
	}
	.tag-x:hover:not(:disabled) {
		background: var(--border-strong);
		color: var(--text);
	}
	.add-tag {
		min-height: 20px;
		padding: 0 7px;
		gap: 3px;
		font-size: 11px;
		border-radius: 999px;
		border-style: dashed;
		background: none;
		color: var(--muted);
	}
	.tag-input {
		min-height: 20px !important;
		width: 90px;
		padding: 0 7px;
		font-size: 11px;
		border-radius: 999px;
	}
	.tag-input.bad {
		border-color: var(--fail);
	}
	.url-row {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-top: 4px;
	}
	.urlbar {
		display: flex;
		flex: 1;
		min-width: 0;
		border: 1px solid var(--border-strong);
		border-radius: var(--radius);
		background: var(--bg);
		transition: border-color 0.12s;
	}
	.urlbar:focus-within {
		border-color: var(--accent);
		box-shadow: 0 0 0 3px #22a88226;
	}
	.urlbar select,
	.urlbar input {
		border: none;
		background: none;
		min-height: 32px;
		box-shadow: none;
	}
	.method-pill {
		flex: none;
		padding: 0 10px;
		font-size: 12px;
		border-right: 1px solid var(--border) !important;
		border-radius: var(--radius) 0 0 var(--radius);
		cursor: pointer;
	}
	.method-pill option {
		color: var(--text);
		background: var(--panel-2);
	}
	.url {
		flex: 1;
		font-size: 12.5px;
	}
	.tabbar {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 10px 0 6px;
		border-bottom: 1px solid var(--border);
	}
	.tabs {
		display: flex;
		gap: 2px;
		overflow-x: auto;
	}
	.tab {
		position: relative;
		min-height: 34px;
		padding: 0 10px;
		gap: 5px;
		border: none;
		border-radius: 0;
		background: none;
		color: var(--muted);
		font-size: 12px;
		white-space: nowrap;
	}
	.tab:hover:not(:disabled) {
		background: none;
		color: var(--text);
	}
	.tab.active {
		color: var(--text);
		box-shadow: inset 0 -2px 0 var(--accent);
	}
	.count {
		font-size: 10px;
		padding: 0 5px;
		border-radius: 999px;
		background: var(--panel-3);
		color: var(--muted);
	}
	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--warn);
	}
	.panel {
		flex: 1;
		min-height: 0;
		overflow: auto;
		padding: 10px 14px 20px;
		display: flex;
		flex-direction: column;
		gap: 8px;
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
		grid-template-columns: minmax(0, 2fr) minmax(0, 3fr) 28px;
		align-items: center;
		border-bottom: 1px solid var(--border);
	}
	.kv-row:last-child {
		border-bottom: none;
	}
	.kv.single .kv-row {
		grid-template-columns: auto minmax(0, 1fr) 28px;
	}
	.kv-row input {
		border: none;
		border-radius: 0;
		background: none;
		min-height: 30px;
		box-shadow: none;
	}
	.kv-row input + input {
		border-left: 1px solid var(--border);
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
	.del {
		justify-self: center;
		opacity: 0;
	}
	.kv-row:hover .del,
	.kv-row:focus-within .del {
		opacity: 1;
	}
	.body-wrap {
		position: relative;
		flex: 1;
		display: flex;
		min-height: 160px;
	}
	.body {
		flex: 1;
		width: 100%;
		resize: none;
		font-size: 12.5px;
		padding-right: 90px;
	}
	.format {
		position: absolute;
		top: 6px;
		right: 6px;
		min-height: 22px;
		padding: 0 8px;
		gap: 4px;
		font-size: 11px;
		color: var(--muted);
		background: var(--panel-2);
		opacity: 0.85;
	}
	.format:hover:not(:disabled) {
		color: var(--text);
		opacity: 1;
	}
	.format.bad {
		color: var(--warn);
		border-color: var(--warn-border);
		background: var(--warn-bg);
	}
	.settings {
		display: grid;
		grid-template-columns: 90px 1fr;
		gap: 12px 10px;
		align-items: start;
	}
	.settings label {
		color: var(--muted);
		padding-top: 5px;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 3px;
	}
	.timeout {
		max-width: 140px;
	}
	.hint {
		color: var(--muted);
		font-size: 11px;
		margin: 0;
	}
</style>
