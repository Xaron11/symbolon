<script lang="ts">
	import '@fontsource-variable/ibm-plex-sans';
	import '@fontsource-variable/jetbrains-mono';
	import '#lib/design/generated/tokens.css';
	import '#lib/design/base.css';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<title>Symbolon</title>
</svelte:head>

{@render children()}
