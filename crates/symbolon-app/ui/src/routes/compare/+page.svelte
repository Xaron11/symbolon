<script lang="ts">
	import { goto } from '$app/navigation';
	import ComparePicker from '#lib/compare/ComparePicker.svelte';
</script>

<svelte:head><title>Compare · Symbolon</title></svelte:head>

<ComparePicker
	navigate={(href: string, options?: { replace: boolean }) => void goto(href, options)}
/>
