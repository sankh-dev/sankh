import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		sveltekit({
			adapter: adapter({
				fallback: '404.html'
			}),
			paths: {
				origin: 'https://sankh.dev'
			}
		})
	],
	server: {
		fs: {
			// docs/format.md lives in the repository root, outside site/.
			allow: ['..']
		}
	}
});
