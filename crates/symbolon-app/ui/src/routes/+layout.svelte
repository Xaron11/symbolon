<script lang="ts">
	import '@fontsource-variable/ibm-plex-sans';
	import '@fontsource-variable/jetbrains-mono';
	import '#lib/design/generated/tokens.css';
	import '#lib/design/base.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { createTauriBackend, provideBackend } from '#lib/api/index.ts';
	import { createScanState, provideScans } from '#lib/scans/scans.svelte.ts';
	import Shell from '#lib/shell/Shell.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	const backend = createTauriBackend();
	provideBackend(backend);
	const scans = createScanState(backend);
	provideScans(scans);
	// Scans keep running in the backend when the window reloads; follow them again.
	void scans.init().catch(() => undefined);
</script>

<svelte:head>
	<title>Symbolon</title>
</svelte:head>

<Shell pathname={page.url.pathname} navigate={(href: string) => void goto(href)} {scans}>
	{@render children()}
</Shell>
