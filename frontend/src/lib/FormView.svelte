<script lang="ts">
	import Icon from './Icon.svelte';
	import type { RequestForm } from './types';

	interface Props {
		form: RequestForm;
		onedit: () => void;
	}

	let { form = $bindable(), onedit }: Props = $props();

	const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];

	let tagsText = $derived(form.tags.join(' '));
	let flagsText = $derived(form.curl.flags.join(' '));

	function edit() {
		onedit();
	}

	function setTags(text: string) {
		form.tags = text.split(/\s+/).filter(Boolean);
		edit();
	}

	function setFlags(text: string) {
		form.curl.flags = text.split(/\s+/).filter(Boolean);
		edit();
	}

	function setBody(text: string) {
		form.curl.body = text === '' ? null : text;
		edit();
	}

	function addHeader() {
		form.curl.headers.push({ name: '', value: '' });
		edit();
	}

	function removeAt(list: unknown[], i: number) {
		list.splice(i, 1);
		edit();
	}
</script>

<div class="form" oninput={edit} onchange={edit}>
	<section class="grid">
		<label for="f-name">Name</label>
		<input id="f-name" bind:value={form.name} />

		<label for="f-desc" class="top">Description</label>
		<textarea
			id="f-desc"
			class="desc"
			rows="2"
			value={form.description ?? ''}
			placeholder="What this request does, what it needs"
			oninput={(e) => (form.description = e.currentTarget.value || null)}
		></textarea>

		<label for="f-tags" class="top">Tags</label>
		<div class="field">
			<input
				id="f-tags"
				value={tagsText}
				placeholder="smoke orders"
				aria-describedby="f-tags-hint"
				onchange={(e) => setTags(e.currentTarget.value)}
			/>
			<p class="hint" id="f-tags-hint">
				Space-separated labels for picking requests in CI, e.g. <code>sankh run --tag smoke</code>. Lowercase
				letters, digits, <code>-</code> and <code>_</code>.
			</p>
		</div>

		<label for="f-timeout" class="top">Timeout</label>
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
	</section>

	<section class="line">
		<select bind:value={form.curl.method} onchange={edit} aria-label="Method">
			{#each METHODS as m (m)}
				<option value={m}>{m}</option>
			{/each}
		</select>
		<input class="mono url" bind:value={form.curl.url} placeholder="$BASE_URL/path" aria-label="URL" />
	</section>
	<p class="hint"><code>$VAR</code> expands from the environment; <code>\$</code> is a literal dollar sign.</p>

	<section>
		<h4>Headers <button class="ghost small" onclick={addHeader}><Icon name="plus" size={11} />add</button></h4>
		{#each form.curl.headers as header, i (i)}
			<div class="pair">
				<input class="mono" bind:value={header.name} placeholder="Name" aria-label="Header name" />
				<input class="mono" bind:value={header.value} placeholder="Value" aria-label="Header value" />
				<button class="icon-btn" title="Remove" aria-label="Remove" onclick={() => removeAt(form.curl.headers, i)}><Icon name="x" size={12} /></button>
			</div>
		{/each}
	</section>

	<section>
		<h4>Body</h4>
		<textarea
			class="mono"
			rows="6"
			value={form.curl.body ?? ''}
			placeholder={'{"name":"test"}'}
			oninput={(e) => setBody(e.currentTarget.value)}
			aria-label="Body"
		></textarea>
	</section>

	<section>
		<h4>
			Assertions <span class="hint">status 201 · json .data.id exists</span>
			<button class="ghost small" onclick={() => (form.expects.push(''), edit())}><Icon name="plus" size={11} />add</button>
		</h4>
		{#each form.expects as _, i (i)}
			<div class="pair single">
				<span class="prefix mono">@expect</span>
				<input class="mono" bind:value={form.expects[i]} aria-label="Assertion" />
				<button class="icon-btn" title="Remove" aria-label="Remove" onclick={() => removeAt(form.expects, i)}><Icon name="x" size={12} /></button>
			</div>
		{/each}
		{#if form.expects.length === 0}
			<p class="hint">No assertions: any 2xx status passes.</p>
		{/if}
	</section>

	<section>
		<h4>
			Captures <span class="hint">TOKEN=.data.token · ID=header X-Id</span>
			<button class="ghost small" onclick={() => (form.captures.push(''), edit())}><Icon name="plus" size={11} />add</button>
		</h4>
		{#each form.captures as _, i (i)}
			<div class="pair single">
				<span class="prefix mono">@capture</span>
				<input class="mono" bind:value={form.captures[i]} aria-label="Capture" />
				<button class="icon-btn" title="Remove" aria-label="Remove" onclick={() => removeAt(form.captures, i)}><Icon name="x" size={12} /></button>
			</div>
		{/each}
	</section>

	<section class="grid">
		<label for="f-flags">curl flags</label>
		<input id="f-flags" class="mono" value={flagsText} onchange={(e) => setFlags(e.currentTarget.value)} />
	</section>
</div>

<style>
	.form {
		padding: 12px 14px 24px;
		display: flex;
		flex-direction: column;
		gap: 14px;
		overflow: auto;
		height: 100%;
	}
	.grid {
		display: grid;
		grid-template-columns: 90px 1fr;
		gap: 6px 10px;
		align-items: center;
	}
	label {
		color: var(--muted);
	}
	label.top {
		align-self: start;
		padding-top: 5px;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 3px;
	}
	.desc {
		field-sizing: content;
		min-height: calc(2lh + 10px);
		max-height: 12lh;
		font-family: inherit;
	}
	.timeout {
		max-width: 140px;
	}
	.line {
		display: flex;
		gap: 6px;
	}
	.url {
		flex: 1;
	}
	h4 {
		margin: 0 0 6px;
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--muted);
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.hint {
		color: var(--muted);
		font-weight: 400;
		font-size: 11px;
		margin: 0;
		text-transform: none;
		letter-spacing: 0;
	}
	.pair {
		display: grid;
		grid-template-columns: 1fr 2fr auto;
		gap: 6px;
		margin-bottom: 4px;
	}
	.pair.single {
		grid-template-columns: auto 1fr auto;
		align-items: center;
	}
	.prefix {
		color: var(--muted);
	}
	textarea {
		width: 100%;
		resize: vertical;
	}
	.small {
		min-height: 20px;
		padding: 0 6px;
		gap: 3px;
		font-size: 11px;
		text-transform: none;
		letter-spacing: 0;
	}
	.pair .icon-btn {
		align-self: center;
	}
</style>
