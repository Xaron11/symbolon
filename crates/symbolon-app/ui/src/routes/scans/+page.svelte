<script lang="ts">
	import { goto } from '$app/navigation';
	import ScansView from '#lib/scans/ScansView.svelte';
</script>

<svelte:head><title>Scans · Symbolon</title></svelte:head>

<ScansView navigate={(href: string) => void goto(href)} />
