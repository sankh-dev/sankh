<script lang="ts">
	import { resolve } from '$app/paths';

	let { data } = $props();
</script>

<svelte:head>
	<title>{data.title} - Sankh docs</title>
	<meta name="description" content={data.description} />
	<meta property="og:title" content="{data.title} - Sankh docs" />
	<meta property="og:description" content={data.description} />
	<meta property="og:url" content="https://sankh.dev/docs/{data.slug}" />
	<link rel="canonical" href="https://sankh.dev/docs/{data.slug}" />
</svelte:head>

<div class="layout">
	<article class="prose">
		<h1>{data.title}</h1>
		{@html data.html}

		<nav class="pager" aria-label="Previous and next page">
			{#if data.prev}
				<a class="prev" href={resolve('/docs/[slug]', { slug: data.prev.slug })}>
					<span>Previous</span>{data.prev.title}
				</a>
			{/if}
			{#if data.next}
				<a class="next" href={resolve('/docs/[slug]', { slug: data.next.slug })}>
					<span>Next</span>{data.next.title}
				</a>
			{/if}
		</nav>
	</article>

	{#if data.toc.length > 1}
		<aside class="toc" aria-label="On this page">
			<div class="caption">On this page</div>
			<ul>
				{#each data.toc as entry (entry.id)}
					<li><a href="#{entry.id}">{entry.text}</a></li>
				{/each}
			</ul>
		</aside>
	{/if}
</div>

<style>
	.layout {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 200px;
		gap: 48px;
	}

	.prose {
		max-width: 760px;
		min-width: 0;
	}

	.prose h1 {
		margin: 0 0 24px;
		font-size: 38px;
		letter-spacing: -0.02em;
	}

	.prose :global(h2) {
		margin: 44px 0 14px;
		padding-top: 8px;
		font-size: 24px;
		scroll-margin-top: 80px;
	}

	.prose :global(h3) {
		margin: 30px 0 10px;
		font-size: 19px;
		scroll-margin-top: 80px;
	}

	.prose :global(:is(h2, h3) code) {
		background: none;
		border: 0;
		padding: 0;
		font-size: 0.9em;
	}

	.prose :global(.anchor) {
		float: left;
		margin-left: -0.9em;
		padding-right: 0.2em;
		color: var(--muted);
		opacity: 0;
		text-decoration: none;
	}

	.prose :global(h2:hover .anchor),
	.prose :global(h3:hover .anchor) {
		opacity: 1;
	}

	.prose :global(p),
	.prose :global(ul),
	.prose :global(ol) {
		margin: 0 0 16px;
	}

	.prose :global(li) {
		margin: 4px 0;
	}

	.prose :global(pre) {
		margin: 0 0 20px;
	}

	.prose :global(table) {
		width: 100%;
		margin: 0 0 20px;
		border-collapse: collapse;
		font-size: 15px;
	}

	.prose :global(th),
	.prose :global(td) {
		text-align: left;
		padding: 8px 12px 8px 0;
		border-bottom: 1px solid var(--border);
		vertical-align: top;
	}

	.prose :global(th) {
		color: var(--muted);
	}

	.prose :global(blockquote) {
		margin: 0 0 16px;
		padding: 4px 16px;
		border-left: 3px solid var(--accent);
		color: var(--muted);
	}

	.pager {
		display: flex;
		justify-content: space-between;
		gap: 16px;
		margin-top: 56px;
		padding-top: 24px;
		border-top: 1px solid var(--border);
	}

	.pager a {
		display: flex;
		flex-direction: column;
		padding: 12px 16px;
		border: 1px solid var(--border);
		border-radius: 8px;
		color: var(--text);
		font-weight: 600;
	}

	.pager a:hover {
		border-color: var(--accent);
		text-decoration: none;
	}

	.pager span {
		color: var(--muted);
		font-size: 13px;
		font-weight: 400;
	}

	.next {
		margin-left: auto;
		text-align: right;
	}

	.toc {
		position: sticky;
		top: 92px;
		align-self: start;
		font-size: 14px;
	}

	.caption {
		color: var(--muted);
		font: 600 12px/1 var(--sans);
		text-transform: uppercase;
		letter-spacing: 0.06em;
		margin-bottom: 12px;
	}

	.toc ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.toc li {
		margin: 6px 0;
	}

	.toc a {
		color: var(--muted);
	}

	.toc a:hover {
		color: var(--text);
	}

	@media (max-width: 1100px) {
		.layout {
			grid-template-columns: 1fr;
		}

		.toc {
			display: none;
		}
	}

	@media (max-width: 560px) {
		.prose h1 {
			margin-bottom: 18px;
			font-size: 30px;
		}

		.prose :global(h2) {
			margin-top: 36px;
			font-size: 21px;
		}

		.prose :global(h3) {
			font-size: 17px;
		}

		.prose :global(.anchor) {
			display: none;
		}

		.prose :global(:is(ul, ol)) {
			padding-left: 22px;
		}

		.prose :global(table) {
			display: block;
			overflow-x: auto;
			font-size: 14px;
		}

		.prose :global(th),
		.prose :global(td) {
			min-width: 7em;
		}

		.prose :global(pre) {
			margin-left: -18px;
			margin-right: -18px;
			border-left: 0;
			border-right: 0;
			border-radius: 0;
		}

		.pager {
			flex-direction: column;
			margin-top: 40px;
		}

		.next {
			margin-left: 0;
		}
	}
</style>
