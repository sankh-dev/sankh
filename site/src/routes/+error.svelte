<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
</script>

<svelte:head>
	<title>{page.status} - Sankh</title>
</svelte:head>

<div class="wrap error">
	<h1>{page.status}</h1>
	<p>{page.status === 404 ? 'This page does not exist.' : page.error?.message}</p>
	<p>
		<a class="btn" href={resolve('/')}>Home</a>
		<a class="btn" href={resolve('/docs/[slug]', { slug: 'quickstart' })}>Docs</a>
	</p>
</div>

<style>
	.error {
		padding-top: 96px;
		text-align: center;
	}

	h1 {
		margin: 0;
		font-size: 72px;
		color: var(--accent);
	}

	p {
		color: var(--muted);
	}

	.btn {
		margin: 0 6px;
	}
</style>
