<script lang="ts">
	import { untrack } from 'svelte';
	import {
		CAPTURE_NAME,
		EXPECT_OPS,
		MASK,
		captureLine,
		captureName,
		expectLine,
		jqLiteral,
		jqPath,
		type AnnotationKind,
		type ExpectOp,
		type JsonPick
	} from './jsonpath';

	interface Props {
		/** The parent remounts this per pick; initial values are taken once. */
		pick: JsonPick;
		onadd: (kind: AnnotationKind, line: string) => void;
		onclose: () => void;
	}

	let { pick, onadd, onclose }: Props = $props();

	const WIDTH = 360;
	const initial = untrack(() => pick);
	const kind = initial.kind;
	const isArray = Array.isArray(initial.value);
	const isContainer = typeof initial.value === 'object' && initial.value !== null;
	const ops = EXPECT_OPS.filter((o) => o !== 'length >' || isArray);
	/** The server masks secrets as `***`, so the shown value is not the real one. */
	const masked = typeof initial.value === 'string' && initial.value.includes(MASK);
	const prefill = isContainer || masked ? '' : jqLiteral(initial.value);

	let path = $state(jqPath(initial.segments));
	let op = $state<ExpectOp>(isContainer || masked ? 'exists' : '==');
	let value = $state(prefill);
	let name = $state(captureName(initial.segments));
	let box = $state<HTMLDivElement>();

	let left = $derived(Math.max(8, Math.min(pick.anchor.left, window.innerWidth - WIDTH - 8)));
	let top = $derived(
		pick.anchor.bottom + 220 > window.innerHeight ? Math.max(8, pick.anchor.top - 210) : pick.anchor.bottom + 6
	);

	let error = $derived.by(() => {
		if (!path.trim().startsWith('.')) return 'Path must start with "."';
		if (kind === 'capture') {
			return CAPTURE_NAME.test(name) ? null : 'Name: uppercase letters, digits and _';
		}
		if (op === 'exists') return null;
		try {
			const v = JSON.parse(value);
			if (op === 'length >' && typeof v !== 'number') return 'Length needs a number';
			if (op === 'matches' && typeof v !== 'string') return 'Regex must be a quoted string';
		} catch {
			return 'Value must be JSON, e.g. 7, "text", true, null';
		}
		return null;
	});

	let line = $derived(
		kind === 'capture' ? captureLine(name, path.trim()) : expectLine(path.trim(), op, value.trim())
	);

	function changeOp(next: ExpectOp) {
		op = next;
		if (next === 'length >') value = '0';
		else if (next === 'matches' && !value.startsWith('"')) value = '"^.+$"';
		else if (!value) value = prefill;
	}

	function submit(e: SubmitEvent) {
		e.preventDefault();
		if (error) return;
		onadd(kind, line);
		onclose();
	}

	function onpointerdown(e: PointerEvent) {
		if (box && !box.contains(e.target as Node)) onclose();
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key !== 'Escape') return;
		e.preventDefault();
		e.stopPropagation();
		onclose();
	}

	function focusFirst(node: HTMLInputElement) {
		node.focus();
		node.select();
	}
</script>

<svelte:window {onpointerdown} {onkeydown} />

<div
	bind:this={box}
	class="pop"
	role="dialog"
	style:left="{left}px"
	style:top="{top}px"
	style:width="{WIDTH}px"
	aria-label={kind === 'expect' ? 'Add assertion' : 'Add capture'}
>
	<form onsubmit={submit}>
		<div class="head">{kind === 'expect' ? 'Add assertion' : 'Add capture'}</div>
		{#if kind === 'capture'}
			<label>
				<span>Name</span>
				<input class="mono" bind:value={name} aria-label="Capture name" {@attach focusFirst} />
			</label>
			<label>
				<span>Path</span>
				<input class="mono" bind:value={path} aria-label="JSON path" />
			</label>
		{:else}
			<label>
				<span>Path</span>
				<input class="mono" bind:value={path} aria-label="JSON path" />
			</label>
			<div class="cond">
				<select
					class="mono"
					value={op}
					onchange={(e) => changeOp(e.currentTarget.value as ExpectOp)}
					aria-label="Operator"
				>
					{#each ops as o (o)}
						<option value={o}>{o}</option>
					{/each}
				</select>
				{#if op !== 'exists'}
					<input class="mono value" bind:value aria-label="Expected value" {@attach focusFirst} />
				{/if}
			</div>
		{/if}
		{#if masked && kind === 'expect'}
			<p class="note">
				This value is a secret, shown masked as <code>***</code>. Comparing against it would fail; check it
				<code>exists</code> or compare to a variable such as <code>"$TOKEN"</code>.
			</p>
		{/if}
		<code class="preview" title={line}># @{kind} {line}</code>
		{#if error}<p class="error">{error}</p>{/if}
		<div class="buttons">
			<span class="hint">Added to the open request; save to keep it.</span>
			<button type="button" onclick={onclose}>Cancel</button>
			<button type="submit" class="primary" disabled={!!error}>Add</button>
		</div>
	</form>
</div>

<style>
	.pop {
		position: fixed;
		z-index: 30;
		padding: 12px;
		background: var(--panel-2);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius);
		box-shadow: var(--shadow);
		animation: pop 0.1s ease-out;
	}
	form {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.head {
		font-weight: 600;
	}
	label {
		display: grid;
		grid-template-columns: 44px 1fr;
		align-items: center;
		gap: 8px;
		font-size: 12px;
		color: var(--muted);
	}
	.cond {
		display: flex;
		gap: 6px;
		padding-left: 52px;
	}
	.cond select {
		flex: none;
	}
	.cond .value {
		flex: 1;
		min-width: 0;
	}
	.preview {
		display: block;
		padding: 6px 8px;
		font-size: 11px;
		background: var(--panel-3);
		border-radius: var(--radius-sm);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.note {
		margin: 0;
		font-size: 11px;
		color: var(--warn);
	}
	.error {
		margin: 0;
		font-size: 12px;
		color: var(--fail);
	}
	.buttons {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.hint {
		flex: 1;
		font-size: 11px;
		color: var(--muted);
	}
	@keyframes pop {
		from {
			opacity: 0;
			transform: translateY(-3px);
		}
	}
</style>
