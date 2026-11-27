<script lang="ts">
	import { goto } from '$app/navigation';
	import LocationDetail from '#lib/locations/LocationDetail.svelte';
	import type { PageProps } from './$types';

	let { params }: PageProps = $props();
</script>

<svelte:head><title>{params.label} · Symbolon</title></svelte:head>

{#key params.label}
	<LocationDetail label={params.label} navigate={(href: string) => void goto(href)} />
{/key}
