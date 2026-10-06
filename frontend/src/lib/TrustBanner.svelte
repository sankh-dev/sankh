<script lang="ts">
	import Icon from './Icon.svelte';
	import type { TrustStatus } from './types';

	interface Props {
		name: string;
		trust: TrustStatus;
		root: string;
		ontrust: () => void;
	}

	let { name, trust, root, ontrust }: Props = $props();
</script>

{#if trust.state !== 'trusted'}
	<div class="banner" role="alert">
		<Icon name={trust.state === 'changed' ? 'alert' : 'shield'} size={16} />
		<div class="text">
			{#if trust.state === 'changed'}
				<strong>{name} changed since you trusted it</strong>
				(git {trust.trusted_head.slice(0, 8)} → {trust.current_head.slice(0, 8)}). Review the changes
				before running anything.
				{#if trust.changed.length}
					<div class="files">
						{#each trust.changed.slice(0, 10) as file (file)}
							<code>{file}</code>
						{/each}
						{#if trust.changed.length > 10}
							<span>and {trust.changed.length - 10} more</span>
						{/if}
					</div>
				{/if}
			{:else}
				<strong>{name} is not trusted.</strong>
				Request files are shell scripts. You can view and edit them, but nothing runs until you trust
				<code>{root}</code>.
			{/if}
		</div>
		<button class="primary" onclick={ontrust}>Trust this folder</button>
	</div>
{/if}

<style>
	.banner {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 8px 14px;
		background: var(--warn-bg);
		border-bottom: 1px solid var(--warn-border);
		color: #f8e3a8;
	}
	.banner > :global(.icon) {
		color: var(--warn);
	}
	.text {
		flex: 1;
	}
	code {
		color: #fff;
	}
	.files {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 10px;
		margin-top: 4px;
		font-size: 0.9em;
	}
	button {
		flex: none;
	}
</style>
