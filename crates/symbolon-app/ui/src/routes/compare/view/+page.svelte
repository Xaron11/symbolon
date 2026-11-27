<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import CompareView from '#lib/compare/CompareView.svelte';
</script>

<svelte:head><title>Compare · Symbolon</title></svelte:head>

{#key page.url.search}
	<CompareView query={page.url.searchParams} navigate={(href: string) => void goto(href)} />
{/key}
