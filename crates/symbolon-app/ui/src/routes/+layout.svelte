<script lang="ts">
	import '@fontsource-variable/ibm-plex-sans';
	import '@fontsource-variable/jetbrains-mono';
	import '#lib/design/generated/tokens.css';
	import '#lib/design/base.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { createTauriBackend, provideBackend, toApiError } from '#lib/api/index.ts';
	import { toasts } from '#lib/feedback/toasts.svelte.ts';
	import { createOpenComparison, provideOpenComparison } from '#lib/compare/open.svelte.ts';
	import { createScanState, provideScans } from '#lib/scans/scans.svelte.ts';
	import Shell from '#lib/shell/Shell.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	const backend = createTauriBackend();
	provideBackend(backend);
	// Comparisons opened before a reload lost their ids with the page; close them.
	void backend.compare.closeAll().catch(() => undefined);
	provideOpenComparison(createOpenComparison(backend));
	const scans = createScanState(backend);
	provideScans(scans);
	// Scans keep running in the backend when the window reloads; follow them again.
	void scans.init().catch((e: unknown) => {
		toasts.show(`Could not follow running scans: ${toApiError(e).message}`, 'error');
	});
</script>

<svelte:head>
	<title>Symbolon</title>
</svelte:head>

<Shell pathname={page.url.pathname} navigate={(href: string) => void goto(href)} {scans}>
	{@render children()}
</Shell>
