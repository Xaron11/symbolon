<script lang="ts">
	import Button from '../Button/Button.svelte';
	import Dialog from './Dialog.svelte';

	let { open = false }: { open?: boolean } = $props();
</script>

<Button onclick={() => (open = true)}>Add location</Button>
<Dialog bind:open title="Add location" description="A folder, drive or phone.">
	<input aria-label="Path" />
	{#snippet footer()}
		<Button onclick={() => (open = false)}>Cancel</Button>
		<Button variant="primary">Add</Button>
	{/snippet}
</Dialog>
