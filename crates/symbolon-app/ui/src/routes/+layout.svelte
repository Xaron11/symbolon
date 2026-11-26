<script lang="ts">
	import '@fontsource-variable/ibm-plex-sans';
	import '@fontsource-variable/jetbrains-mono';
	import '#lib/design/generated/tokens.css';
	import '#lib/design/base.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import Shell from '#lib/shell/Shell.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<title>Symbolon</title>
</svelte:head>

<Shell pathname={page.url.pathname} navigate={(href: string) => void goto(href)}>
	{@render children()}
</Shell>
