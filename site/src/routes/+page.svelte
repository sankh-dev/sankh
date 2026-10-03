<script lang="ts">
	import { asset, resolve } from '$app/paths';
	import CopyCommand from '#lib/CopyCommand.svelte';
	import { GITHUB_URL } from '#lib/docs.ts';

	let { data } = $props();

	const INSTALL = 'curl -fsSL https://sankh.dev/install.sh | sh';

	const runLines = [
		{ name: 'Log in', status: 200, ms: 38, path: 'auth/01-login.sh', capture: 'TOKEN=***' },
		{ name: 'List pets', status: 200, ms: 6, path: 'pets/01-list.sh' },
		{ name: 'Create pet', status: 201, ms: 7, path: 'pets/02-create.sh', capture: 'PET_ID=1' },
		{ name: 'Get pet', status: 200, ms: 5, path: 'pets/03-get.sh' },
		{ name: 'Delete pet', status: 204, ms: 5, path: 'pets/04-delete.sh' },
		{ name: 'Deleted pet is gone', status: 404, ms: 4, path: 'pets/05-get-deleted.sh' }
	];

	const installOptions = [
		{ label: 'Shell (Linux, macOS)', command: INSTALL },
		{ label: 'Pinned version', command: 'curl -fsSL https://sankh.dev/install.sh | SANKH_VERSION=0.2.1 sh' },
		{ label: 'Homebrew', command: 'brew install sankh-dev/tap/sankh' },
		{
			label: 'PowerShell (Windows)',
			prompt: '>',
			command: `powershell -c "irm ${GITHUB_URL}/releases/latest/download/sankh-installer.ps1 | iex"`
		}
	];
</script>

<svelte:head>
	<title>Sankh: plain-file API requests for CI and your browser</title>
	<meta
		name="description"
		content="Sankh is a lightweight request manager. A collection is a git folder of curl scripts; one binary runs them in CI or serves a local web UI, and a desktop app opens the same UI in a native window."
	/>
	<meta property="og:title" content="Sankh: blow the conch, run your APIs" />
	<meta
		property="og:description"
		content="API collections as plain .sh files. Run them in CI, in a local web UI, or in the desktop app."
	/>
	<meta property="og:url" content="https://sankh.dev/" />
	<link rel="canonical" href="https://sankh.dev/" />
</svelte:head>

<section class="hero">
	<div class="wrap hero-grid">
		<div class="hero-copy">
			<img class="hero-logo" src={asset('logo.svg')} alt="Sankh logo" width="72" height="72" />
			<h1>Blow the conch.<br /><span>Run your APIs.</span></h1>
			<p class="lede">
				Sankh is a very lightweight request manager. A collection is a plain folder of <code>.sh</code>
				files, each holding one <code>curl</code> command. One binary runs them in CI or serves a small
				web UI, and the desktop app opens the same UI in a native window.
			</p>
			<CopyCommand command={INSTALL} />
			<div class="cta">
				<a class="btn primary" href={resolve('/docs/[slug]', { slug: 'quickstart' })}>Get started</a>
				<a class="btn" href={GITHUB_URL} rel="external">View on GitHub</a>
			</div>
		</div>
		<div class="hero-code" aria-label="Example request file">
			<div class="file-tab">pets/02-create.sh</div>
			{@html data.requestFile}
		</div>
	</div>
</section>

<section class="section">
	<div class="wrap">
		<h2>A collection is a folder of curl</h2>
		<p class="section-lede">
			Annotations live in shell comments, so every file still runs on its own, with or without Sankh.
			Your API collection is a git repo that runs anywhere. The UI is optional.
		</p>
		<div class="split">
			<div class="stack">
				<div class="caption">Layout</div>
				{@html data.layout}
				<div class="caption">Works without Sankh</div>
				{@html data.standalone}
			</div>
			<div class="stack">
				<div class="caption">sankh run</div>
				<pre class="terminal"><code
						><span class="dim">$</span> sankh run examples/petstore --env dev
<b>sankh · petstore · env dev</b> <span class="dim">(6 requests)</span>
{#each runLines as line (line.path)}<span class="ok">✓</span> {line.name}  <span class="dim"
								>{line.status}  {line.ms}ms  {line.path}</span
							>
{#if line.capture}    <span class="dim">captured {line.capture}</span>
{/if}{/each}
<span class="ok">6 passed</span>  <span class="dim">72ms</span></code
					></pre>
			</div>
		</div>
	</div>
</section>

<section class="section">
	<div class="wrap">
		<h2>Run it in CI, a browser, or a window</h2>
		<div class="cards">
			<article class="card">
				<h3>Built for CI</h3>
				<p>
					<code>sankh run</code> filters by folder and tag, writes JUnit or JSON reports, and exits
					non-zero on failure. Values captured from one response feed the next request.
				</p>
			</article>
			<article class="card">
				<h3>An optional web UI</h3>
				<p>
					<code>sankh serve</code> opens a file tree, form and raw editors, and live results. The server
					runs the requests, not the browser, so there are no CORS workarounds.
				</p>
			</article>
			<article class="card">
				<h3>A desktop app</h3>
				<p>
					The same UI in a native window for macOS, Windows and Linux, with a native folder picker.
					It edits the same files and runs requests on your machine.
					<a href={resolve('/docs/[slug]', { slug: 'desktop' })}>Download</a>
				</p>
			</article>
		</div>
		<figure class="shot">
			<img
				src={asset('serve.png')}
				alt="The Sankh web UI showing the petstore collection, the Create pet request editor, and a passing run"
				width="1441"
				height="656"
				loading="lazy"
			/>
			<figcaption><code>sankh serve examples/petstore</code></figcaption>
		</figure>
	</div>
</section>

<section class="section">
	<div class="wrap">
		<h2>All your collections, side by side</h2>
		<p class="section-lede">
			Plain files in git, no proprietary export format. Keep several collections open at once, each
			with its own environments, captured values, and trust.
		</p>
		<div class="cards">
			<article class="card">
				<h3>Workspaces</h3>
				<p>
					<code>sankh workspace add</code> keeps folders in your workspace, so plain
					<code>sankh serve</code> and the desktop app open them all. Removing one only unlinks it.
				</p>
			</article>
			<article class="card">
				<h3>Scratch</h3>
				<p>
					A built-in collection for trying requests without a folder. Paste a <code>curl</code> command,
					run it, and <strong>Copy to</strong> a real collection when it is worth keeping.
				</p>
			</article>
			<article class="card">
				<h3>Import from Postman</h3>
				<p>
					<code>sankh import postman</code> turns a collection and its environments into
					<code>.sh</code> files. Common test scripts become assertions; secrets never reach disk.
				</p>
			</article>
		</div>
		<p class="more">
			<a href={resolve('/docs/[slug]', { slug: 'workspaces' })}>Workspaces and Scratch</a>
			<span class="dim">·</span>
			<a href={resolve('/docs/[slug]', { slug: 'import' })}>Importing from Postman</a>
		</p>
	</div>
</section>

<section class="section">
	<div class="wrap split">
		<div>
			<h2>Drop it into any pipeline</h2>
			<p class="section-lede">
				Install with one line, run the smoke folder, publish a JUnit report. Exit codes tell your CI
				exactly what went wrong.
			</p>
			<table class="exit">
				<thead><tr><th>Exit</th><th>Meaning</th></tr></thead>
				<tbody>
					<tr><td>0</td><td>All requests passed</td></tr>
					<tr><td>1</td><td>An assertion, capture, or request failed</td></tr>
					<tr><td>2</td><td>Usage or configuration error</td></tr>
					<tr><td>3</td><td>The collection is not trusted</td></tr>
				</tbody>
			</table>
		</div>
		<div class="stack">
			<div class="caption">GitHub Actions</div>
			{@html data.ci}
			<a href={resolve('/docs/[slug]', { slug: 'ci' })}>More on running in CI</a>
		</div>
	</div>
</section>

<section class="section">
	<div class="wrap">
		<h2>Safe by default</h2>
		<div class="cards">
			<article class="card">
				<h3>Trust before running</h3>
				<p>
					Request files are scripts, so a cloned collection never runs until you trust it. In a git repo
					trust is tied to the HEAD commit and lapses when it changes.
				</p>
			</article>
			<article class="card">
				<h3>Secrets redacted</h3>
				<p>
					Values of variables named like <code>TOKEN</code>, <code>SECRET</code>, <code>KEY</code> or
					<code>PASSWORD</code> are replaced with <code>***</code> in output, reports, and the UI.
				</p>
			</article>
			<article class="card">
				<h3>Local first, no telemetry</h3>
				<p>
					The UI binds to <code>127.0.0.1</code> and rejects foreign hosts and cross-origin calls. Sankh
					makes no network requests other than the ones in your files.
				</p>
			</article>
		</div>
		<p class="more">
			<a href={resolve('/docs/[slug]', { slug: 'trust-secrets' })}>How trust and redaction work</a>
		</p>
	</div>
</section>

<section class="section" id="install">
	<div class="wrap install">
		<div>
			<h2>Install</h2>
			<p class="section-lede">
				Sankh needs <code>curl</code> and a POSIX shell at run time. On Windows, use Git Bash or WSL.
			</p>
			<ul class="install-list">
				{#each installOptions as opt (opt.label)}
					<li>
						<div class="caption">{opt.label}</div>
						<CopyCommand command={opt.command} prompt={opt.prompt} />
					</li>
				{/each}
			</ul>
			<div class="desktop">
				<div>
					<div class="caption">Desktop app</div>
					<p>
						Installers for macOS (<code>.dmg</code>), Windows (<code>.exe</code>, <code>.msi</code>) and
						Linux (<code>.AppImage</code>, <code>.deb</code>). Not code signed yet; see the
						<a href={resolve('/docs/[slug]', { slug: 'desktop' })}>desktop docs</a> for first launch.
					</p>
				</div>
				<a class="btn" href="{GITHUB_URL}/releases/latest" rel="external">Download</a>
			</div>
		</div>
		<div class="stack">
			<div class="caption">Then</div>
			{@html data.quickstart}
			<a href={resolve('/docs/[slug]', { slug: 'quickstart' })}>Read the quickstart</a>
		</div>
	</div>
</section>

<style>
	.hero {
		padding: 72px 0 56px;
		background:
			radial-gradient(900px 400px at 15% -10%, color-mix(in srgb, var(--accent) 22%, transparent), transparent),
			var(--bg);
		border-bottom: 1px solid var(--border);
	}

	.hero-grid {
		display: grid;
		grid-template-columns: 1.05fr 1fr;
		gap: 48px;
		align-items: center;
	}

	.hero-logo {
		display: block;
		margin-bottom: 20px;
	}

	h1 {
		margin: 0 0 18px;
		font-size: clamp(38px, 6vw, 58px);
		line-height: 1.05;
		letter-spacing: -0.02em;
	}

	h1 span {
		color: var(--accent);
	}

	.lede {
		margin: 0 0 24px;
		color: var(--muted);
		font-size: 18px;
		max-width: 34em;
	}

	.cta {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
		margin-top: 20px;
	}

	.hero-code {
		min-width: 0;
		box-shadow: 0 20px 60px rgb(0 0 0 / 0.35);
		border-radius: 10px;
	}

	.file-tab {
		display: inline-block;
		padding: 6px 14px;
		font: 12px var(--mono);
		color: var(--muted);
		background: var(--panel);
		border: 1px solid var(--border);
		border-bottom: 0;
		border-radius: 8px 8px 0 0;
	}

	.hero-code :global(pre) {
		border-top-left-radius: 0;
	}

	.section {
		padding: 72px 0 0;
	}

	h2 {
		margin: 0 0 12px;
		font-size: 30px;
		letter-spacing: -0.01em;
	}

	.section-lede {
		margin: 0 0 28px;
		color: var(--muted);
		max-width: 44em;
	}

	.split {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 32px;
		align-items: start;
	}

	.split > :global(*) {
		min-width: 0;
	}

	.stack {
		display: flex;
		flex-direction: column;
		gap: 10px;
		min-width: 0;
	}

	.caption {
		color: var(--muted);
		font: 600 12px/1 var(--sans);
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.stack .caption:not(:first-child) {
		margin-top: 12px;
	}

	.terminal {
		font-size: 13.5px;
		white-space: pre;
	}

	.terminal b {
		font-weight: 700;
	}

	.dim {
		color: var(--muted);
	}

	.ok {
		color: var(--ok);
	}

	.cards {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 20px;
		margin-top: 24px;
	}

	.card {
		padding: 22px;
		background: var(--panel);
		border: 1px solid var(--border);
		border-radius: 12px;
	}

	.card h3 {
		margin: 0 0 8px;
		font-size: 18px;
	}

	.card p {
		margin: 0;
		color: var(--muted);
		font-size: 15px;
	}

	.shot {
		margin: 32px 0 0;
	}

	.shot img {
		display: block;
		width: 100%;
		height: auto;
		border: 1px solid var(--border);
		border-radius: 12px;
		box-shadow: 0 20px 60px rgb(0 0 0 / 0.35);
	}

	.shot figcaption {
		margin-top: 10px;
		text-align: center;
		color: var(--muted);
		font-size: 14px;
	}

	.exit {
		border-collapse: collapse;
		font-size: 15px;
	}

	.exit th,
	.exit td {
		text-align: left;
		padding: 8px 18px 8px 0;
		border-bottom: 1px solid var(--border);
	}

	.exit th {
		color: var(--muted);
		font-weight: 600;
	}

	.exit td:first-child {
		font-family: var(--mono);
		color: var(--accent);
	}

	.more {
		margin: 20px 0 0;
	}

	.install {
		display: flex;
		flex-direction: column;
		gap: 32px;
	}

	.install-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 16px 24px;
	}

	.install-list .caption {
		margin-bottom: 8px;
	}

	.desktop {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 24px;
		margin-top: 24px;
		padding: 18px 22px;
		background: var(--panel);
		border: 1px solid var(--border);
		border-radius: 12px;
	}

	.desktop p {
		margin: 8px 0 0;
		color: var(--muted);
		font-size: 15px;
	}

	.desktop .btn {
		flex: none;
	}

	@media (max-width: 860px) {
		.hero-grid,
		.split,
		.cards,
		.install-list {
			grid-template-columns: minmax(0, 1fr);
		}

		.hero-grid > * {
			min-width: 0;
		}

		.hero {
			padding-top: 48px;
		}
	}

	@media (max-width: 560px) {
		.hero {
			padding: 32px 0 40px;
		}

		.hero-grid {
			gap: 36px;
		}

		.hero-logo {
			width: 56px;
			height: 56px;
			margin-bottom: 16px;
		}

		.lede {
			font-size: 16px;
		}

		.cta .btn {
			flex: 1 1 0;
			justify-content: center;
			padding: 12px 14px;
			white-space: nowrap;
		}

		.section {
			padding-top: 56px;
		}

		h2 {
			font-size: 25px;
		}

		.section-lede {
			margin-bottom: 20px;
		}

		.terminal {
			font-size: 12.5px;
		}

		.card {
			padding: 18px;
		}

		.exit {
			width: 100%;
		}

		.desktop {
			flex-direction: column;
			align-items: stretch;
		}

		.desktop .btn {
			justify-content: center;
		}

		.exit th,
		.exit td {
			padding-right: 12px;
		}
	}
</style>
