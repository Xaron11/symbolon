<script lang="ts">
	import Combobox from './Combobox.svelte';

	let value = $state<string | undefined>();
	const options = ['laptop', 'backup', 'backup-old', 'phone'].map((v) => ({ value: v, label: v }));
</script>

<Combobox label="Location" bind:value {options} />
<output data-testid="value">{value ?? ''}</output>
