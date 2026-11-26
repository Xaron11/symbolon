<script lang="ts">
	import Tooltip from './Tooltip.svelte';
</script>

<Tooltip text="Rescan all locations">
	{#snippet children(props)}
		<button type="button" {...props}>Rescan</button>
	{/snippet}
</Tooltip>
