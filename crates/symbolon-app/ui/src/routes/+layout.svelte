<script lang="ts">
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<title>Symbolon</title>
</svelte:head>

{@render children()}

<style>
	:global(:root) {
		color-scheme: light dark;
		--bg: #f7f7f5;
		--fg: #1c1c1a;
		--muted: #6b6b66;
		font-family: system-ui, sans-serif;
	}

	@media (prefers-color-scheme: dark) {
		:global(:root) {
			--bg: #161615;
			--fg: #ececea;
			--muted: #9a9a94;
		}
	}

	:global(body) {
		margin: 0;
		background: var(--bg);
		color: var(--fg);
	}
</style>
