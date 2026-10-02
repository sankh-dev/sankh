<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { docPages } from '#lib/docs.ts';

	let { children } = $props();
</script>

<div class="wrap docs">
	<nav class="sidebar" aria-label="Documentation">
		<div class="caption">Docs</div>
		<ul>
			{#each docPages as doc (doc.slug)}
				{@const href = resolve('/docs/[slug]', { slug: doc.slug })}
				<li>
					<a {href} aria-current={page.url.pathname === href ? 'page' : undefined}>{doc.title}</a>
				</li>
			{/each}
		</ul>
	</nav>
	<div class="content">
		{@render children()}
	</div>
</div>

<style>
	.docs {
		display: grid;
		grid-template-columns: 200px minmax(0, 1fr);
		gap: 48px;
		padding-top: 40px;
	}

	.sidebar {
		position: sticky;
		top: 92px;
		align-self: start;
	}

	.caption {
		color: var(--muted);
		font: 600 12px/1 var(--sans);
		text-transform: uppercase;
		letter-spacing: 0.06em;
		margin-bottom: 12px;
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	li a {
		display: block;
		padding: 6px 12px;
		margin-left: -12px;
		border-radius: 6px;
		color: var(--muted);
		font-size: 15px;
	}

	li a:hover {
		color: var(--text);
		text-decoration: none;
	}

	li a[aria-current='page'] {
		color: var(--text);
		background: var(--panel-2);
	}

	@media (max-width: 860px) {
		.docs {
			grid-template-columns: minmax(0, 1fr);
			gap: 24px;
			padding-top: 24px;
		}

		.sidebar {
			position: static;
			padding-bottom: 16px;
			border-bottom: 1px solid var(--border);
		}

		ul {
			display: flex;
			gap: 6px;
			margin: 0 -18px;
			padding: 0 18px;
			overflow-x: auto;
			scrollbar-width: none;
		}

		li {
			flex-shrink: 0;
		}

		li a {
			margin-left: 0;
			padding: 8px 14px;
			border: 1px solid var(--border);
			border-radius: 999px;
			white-space: nowrap;
		}

		li a[aria-current='page'] {
			border-color: var(--accent);
		}
	}
</style>
