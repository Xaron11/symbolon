<script lang="ts">
	import '@fontsource-variable/ibm-plex-sans';
	import '@fontsource-variable/jetbrains-mono';
	import '#lib/design/generated/tokens.css';
	import '#lib/design/base.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { createTauriBackend, provideBackend } from '#lib/api/index.ts';
	import Shell from '#lib/shell/Shell.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	const backend = createTauriBackend();
	provideBackend(backend);
	// Comparisons opened before a reload lost their ids with the page; close them.
	void backend.compare.closeAll().catch(() => undefined);
</script>

<svelte:head>
	<title>Symbolon</title>
</svelte:head>

<Shell pathname={page.url.pathname} navigate={(href: string) => void goto(href)}>
	{@render children()}
</Shell>
