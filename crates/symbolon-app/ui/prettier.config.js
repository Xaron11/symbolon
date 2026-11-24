// The plugin is imported rather than named so it resolves from this package even when
// Prettier runs from the repository root (treefmt).
import * as svelte from 'prettier-plugin-svelte';

/** @type {import("prettier").Config} */
const config = {
	useTabs: true,
	singleQuote: true,
	trailingComma: 'none',
	printWidth: 100,
	plugins: [svelte],
	overrides: [{ files: '*.svelte', options: { parser: 'svelte' } }]
};

export default config;
