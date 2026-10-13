<script lang="ts">
	import Icon from './Icon.svelte';
	import { STATUS_TEXT, needsValue, refs, type VarStatus } from './vars';

	interface Props {
		/** Text whose `$VAR` references are checked. */
		text: string;
		varStatus: (name: string) => VarStatus;
		onsetvar: (name: string, anchor: DOMRect) => void;
	}

	let { text, varStatus, onsetvar }: Props = $props();

	let flagged = $derived(
		refs(text)
			.map((name) => ({ name, status: varStatus(name) }))
			.filter((v) => needsValue(v.status))
	);
</script>

{#if flagged.length}
	<div class="chips" role="group" aria-label="Variables without a value">
		<Icon name="alert" size={12} class="warn-icon" />
		{#each flagged as v (v.name)}
			<button
				class={['chip', v.status]}
				title="{v.name}: {STATUS_TEXT[v.status]}. Click to set it in .env.local"
				onclick={(e) => onsetvar(v.name, e.currentTarget.getBoundingClientRect())}
			>
				<span class="mono">{v.name}</span>
				<span class="state">{v.status === 'missing' ? 'missing' : 'unset'}</span>
			</button>
		{/each}
	</div>
{/if}

<style>
	.chips {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px;
		min-width: 0;
	}
	.chips :global(.warn-icon) {
		color: var(--warn);
		margin-right: 2px;
	}
	.chip {
		min-height: 20px;
		padding: 0 7px;
		gap: 5px;
		font-size: 11px;
		border-radius: 999px;
		background: var(--warn-bg);
		border-color: var(--warn-border);
		color: var(--warn);
	}
	.chip.missing {
		background: var(--fail-bg);
		border-color: #5c2629;
		color: var(--fail);
	}
	.chip:hover:not(:disabled) {
		background: var(--panel-3);
		border-color: currentColor;
	}
	.chip .mono {
		font-size: 11px;
	}
	.state {
		opacity: 0.75;
	}
</style>
