import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
	plugins: [svelte()],
	server: {
		proxy: {
			// Keep the browser's Host so it matches Origin; the server rejects
			// mismatched POSTs as cross-origin.
			'/api': { target: 'http://localhost:4747', changeOrigin: false }
		}
	},
	build: {
		outDir: 'dist',
		emptyOutDir: true,
		chunkSizeWarningLimit: 1200
	}
});
