<script lang="ts">
	let { command, prompt = '$' }: { command: string; prompt?: string } = $props();

	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout> | undefined;

	async function copy() {
		await navigator.clipboard.writeText(command);
		copied = true;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = false), 1500);
	}
</script>

<div class="cmd">
	<code><span class="prompt" aria-hidden="true">{prompt}</span> {command}</code>
	<button type="button" onclick={copy} aria-label="Copy command">
		{copied ? 'Copied' : 'Copy'}
	</button>
	<span class="visually-hidden" aria-live="polite">{copied ? 'Copied to clipboard' : ''}</span>
</div>

<style>
	.cmd {
		display: flex;
		align-items: center;
		gap: 12px;
		max-width: 100%;
		padding: 10px 10px 10px 16px;
		background: var(--panel);
		border: 1px solid var(--border);
		border-radius: 10px;
	}

	code {
		flex: 1;
		min-width: 0;
		overflow-x: auto;
		white-space: nowrap;
		scrollbar-width: none;
		font-size: 14px;
	}

	.prompt {
		color: var(--accent);
		user-select: none;
	}

	button {
		flex-shrink: 0;
		min-width: 72px;
		padding: 6px 12px;
		border-radius: 6px;
		border: 1px solid var(--border);
		background: var(--panel-2);
		color: var(--text);
		font: 500 13px/1 var(--sans);
		cursor: pointer;
	}

	button:hover {
		border-color: var(--accent);
	}

	@media (max-width: 560px) {
		.cmd {
			gap: 8px;
			padding: 8px 8px 8px 12px;
		}

		code {
			font-size: 13px;
		}

		button {
			min-width: 64px;
			padding: 8px 10px;
		}
	}
</style>
