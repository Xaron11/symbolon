<script lang="ts">
	import { goto } from '$app/navigation';
	import ScanDetail from '#lib/scans/ScanDetail.svelte';
	import type { PageProps } from './$types';

	let { params }: PageProps = $props();
	const scanId = $derived(Number(params.id));
</script>

<svelte:head><title>Scan · Symbolon</title></svelte:head>

{#key scanId}
	<ScanDetail {scanId} navigate={(href: string) => void goto(href)} />
{/key}
