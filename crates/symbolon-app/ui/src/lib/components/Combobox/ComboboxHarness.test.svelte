<script lang="ts">
	import Combobox from './Combobox.svelte';

	let { initial }: { initial?: string } = $props();
	// svelte-ignore state_referenced_locally
	let value = $state<string | undefined>(initial);
	const options = ['laptop', 'backup', 'backup-old', 'phone'].map((v) => ({ value: v, label: v }));
</script>

<Combobox label="Location" bind:value {options} />
<output data-testid="value">{value ?? ''}</output>
<button type="button" onclick={() => (value = undefined)}>Clear</button>
