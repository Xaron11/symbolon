<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import LocationsView from '#lib/locations/LocationsView.svelte';
</script>

<svelte:head><title>Locations · Symbolon</title></svelte:head>

<LocationsView
	navigate={(href: string) => void goto(href)}
	openAdd={page.url.searchParams.has('add')}
	onaddopened={() => {
		// Drop the request so the palette can ask again and Back does not reopen the dialog.
		void goto('/locations', { replace: true, reset: false });
	}}
/>
