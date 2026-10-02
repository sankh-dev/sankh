<script lang="ts">
	import '#lib/theme.css';
	import { asset, resolve } from '$app/paths';
	import { page } from '$app/state';
	import { GITHUB_URL } from '#lib/docs.ts';

	let { children } = $props();

	const inDocs = $derived(page.url.pathname.startsWith(resolve('/docs')));
</script>

<svelte:head>
	<meta property="og:site_name" content="Sankh" />
	<meta property="og:type" content="website" />
	<meta property="og:image" content="https://sankh.dev/og.png" />
	<meta name="twitter:card" content="summary_large_image" />
</svelte:head>

<header class="site-header">
	<div class="wrap bar">
		<a class="brand" href={resolve('/')}>
			<img src={asset('logo.svg')} alt="" width="28" height="28" />
			<span>Sankh</span>
		</a>
		<nav aria-label="Main">
			<a href={resolve('/docs/[slug]', { slug: 'quickstart' })} aria-current={inDocs ? 'page' : undefined}>
				Docs
			</a>
			<a href={GITHUB_URL} rel="external">GitHub</a>
		</nav>
	</div>
</header>

<main>
	{@render children()}
</main>

<footer class="site-footer">
	<div class="wrap foot">
		<p>
			Dual-licensed under
			<a href="{GITHUB_URL}/blob/main/LICENSE-MIT">MIT</a> or
			<a href="{GITHUB_URL}/blob/main/LICENSE-APACHE">Apache-2.0</a>.
			No telemetry.
		</p>
		<p class="pron">
			<em>Sankh</em> (<span lang="hi">&#2358;&#2306;&#2326;</span>, "shankh") is the conch shell blown to
			announce a beginning.
		</p>
	</div>
</footer>

<style>
	.site-header {
		position: sticky;
		top: 0;
		z-index: 10;
		background: color-mix(in srgb, var(--bg) 85%, transparent);
		backdrop-filter: blur(8px);
		border-bottom: 1px solid var(--border);
	}

	.bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 60px;
	}

	.brand {
		display: flex;
		align-items: center;
		gap: 10px;
		color: var(--text);
		font-weight: 700;
		font-size: 18px;
	}

	.brand:hover {
		text-decoration: none;
	}

	nav {
		display: flex;
		gap: 24px;
	}

	nav a {
		color: var(--muted);
		font-weight: 500;
	}

	nav a:hover,
	nav a[aria-current='page'] {
		color: var(--text);
		text-decoration: none;
	}

	main {
		min-height: calc(100vh - 60px - 120px);
	}

	.site-footer {
		border-top: 1px solid var(--border);
		margin-top: 80px;
		color: var(--muted);
		font-size: 14px;
	}

	.foot {
		display: flex;
		flex-wrap: wrap;
		justify-content: space-between;
		gap: 8px 32px;
		padding-top: 28px;
		padding-bottom: 28px;
	}

	.foot p {
		margin: 0;
	}

	@media (max-width: 560px) {
		.bar {
			height: 56px;
		}

		nav {
			gap: 18px;
		}

		.site-footer {
			margin-top: 56px;
		}

		.foot {
			flex-direction: column;
			padding-top: 22px;
			padding-bottom: 22px;
		}
	}
</style>
