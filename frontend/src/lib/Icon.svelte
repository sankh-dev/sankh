<script module lang="ts">
	const PATHS = {
		plus: 'M12 5v14M5 12h14',
		play: 'M7 4.5v15l12-7.5z',
		stop: 'M6 6h12v12H6z',
		more: 'M12 6h.01M12 12h.01M12 18h.01',
		chevron: 'M9 6l6 6-6 6',
		x: 'M6 6l12 12M18 6L6 18',
		trash: 'M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3',
		copy: 'M9 9h11v11H9zM5 15H4V4h11v1',
		terminal: 'M4 17l6-5-6-5M12 19h8',
		folder: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
		upload: 'M12 16V4M7 9l5-5 5 5M4 16v3a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-3',
		check: 'M5 12.5l4.5 4.5L19 7.5',
		arrow: 'M5 12h14M13 6l6 6-6 6',
		alert: 'M12 9v4M12 17h.01M10.3 3.9L2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z',
		shield: 'M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z',
		sliders: 'M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0M14 4v4M8 10v4M16 16v4',
		eraser: 'M16 3l5 5-11 11H5l-2-2 13-14zM14 19h7',
		unlink: 'M9 15l-2 2a3 3 0 0 1-4-4l2-2M15 9l2-2a3 3 0 0 1 4 4l-2 2M8 3v3M3 8h3M16 21v-3M21 16h-3',
		save: 'M5 3h11l3 3v13a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2zM8 3v5h7V3M8 21v-7h8v7',
		eye: 'M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12zM12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z',
		'eye-off':
			'M3 3l18 18M10.6 10.6a2 2 0 0 0 2.8 2.8M9.9 5.1A9.8 9.8 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-3.2 4.2M6.6 6.6A17 17 0 0 0 2 12s3.5 7 10 7a9.7 9.7 0 0 0 5.4-1.6',
		lock: 'M6 11h12v10H6zM8 11V7a4 4 0 0 1 8 0v4',
		collapse: 'M7 4l5 5 5-5M7 20l5-5 5 5',
		expand: 'M7 9l5-5 5 5M7 15l5 5 5-5',
		refresh: 'M21 4v6h-6M3 20v-6h6M20.5 10A9 9 0 0 0 6 5.3M3.5 14A9 9 0 0 0 18 18.7',
		braces:
			'M8 4H7a2 2 0 0 0-2 2v4l-2 2 2 2v4a2 2 0 0 0 2 2h1M16 4h1a2 2 0 0 1 2 2v4l2 2-2 2v4a2 2 0 0 1-2 2h-1'
	} as const;

	export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
	interface Props {
		name: IconName;
		size?: number;
		class?: string;
	}

	let { name, size = 14, class: className }: Props = $props();
</script>

<svg
	class={['icon', className]}
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill={name === 'play' || name === 'stop' ? 'currentColor' : 'none'}
	stroke="currentColor"
	stroke-width={name === 'more' ? 3 : 2}
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
>
	<path d={PATHS[name]} />
</svg>

<style>
	.icon {
		flex: none;
		display: block;
	}
</style>
