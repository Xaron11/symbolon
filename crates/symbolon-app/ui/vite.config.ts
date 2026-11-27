import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { svelteTesting } from '@testing-library/svelte/vite';
import { defineConfig } from 'vitest/config';

const host = process.env['TAURI_DEV_HOST'];

export default defineConfig(({ command }) => ({
	plugins: [
		sveltekit({
			// In dev, styles ride along in each component's module. As separate virtual CSS modules
			// they could be requested before their component was compiled (after a dev server
			// restart under a webview that kept the page), and were then served as raw source.
			emitCss: command === 'build',
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
		projects: [
			{ extends: true, test: { name: 'scripts', include: ['scripts/**/*.test.ts'] } },
			{
				extends: true,
				plugins: [svelteTesting()],
				test: {
					name: 'ui',
					include: ['src/**/*.test.ts'],
					environment: 'jsdom',
					setupFiles: ['src/test/setup.ts']
				}
			}
		]
	},
	clearScreen: false,
	server: {
		port: 5173,
		strictPort: true,
		host: host ?? false,
		...(host && { hmr: { protocol: 'ws', host, port: 5174 } })
	}
}));
