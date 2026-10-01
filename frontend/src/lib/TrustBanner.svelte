<script lang="ts">
	import type { TrustStatus } from './types';

	interface Props {
		trust: TrustStatus;
		root: string;
		ontrust: () => void;
	}

	let { trust, root, ontrust }: Props = $props();
</script>

{#if trust.state !== 'trusted'}
	<div class="banner" role="alert">
		<div>
			{#if trust.state === 'changed'}
				<strong>This collection changed since you trusted it</strong>
				(git {trust.trusted_head.slice(0, 8)} → {trust.current_head.slice(0, 8)}). Review the changes
				before running anything.
			{:else}
				<strong>This folder is not trusted.</strong>
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
		justify-content: space-between;
		gap: 16px;
		padding: 8px 14px;
		background: #3a2f12;
		border-bottom: 1px solid #6b5520;
		color: #f8e3a8;
	}
	code {
		color: #fff;
	}
	button {
		flex: none;
	}
</style>
