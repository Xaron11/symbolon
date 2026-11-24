import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

const host = process.env['TAURI_DEV_HOST'];

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// Tauri serves a static bundle; every route falls back to the SPA shell.
			adapter: adapter({ fallback: 'index.html' })
		})
	],
	test: {
		include: ['src/**/*.test.ts']
	},
	clearScreen: false,
	server: {
		port: 5173,
		strictPort: true,
		host: host ?? false,
		...(host && { hmr: { protocol: 'ws', host, port: 5174 } })
	}
});
