<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import LocationsView from '#lib/locations/LocationsView.svelte';
</script>

<svelte:head><title>Locations · Symbolon</title></svelte:head>

{#key page.url.search}
	<LocationsView
		navigate={(href: string) => void goto(href)}
		openAdd={page.url.searchParams.has('add')}
	/>
{/key}
