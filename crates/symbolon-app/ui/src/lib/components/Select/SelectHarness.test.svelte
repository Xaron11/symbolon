<script lang="ts">
	import Select from './Select.svelte';

	let value = $state<'blake3' | 'xxh3' | 'sha256'>('blake3');
	const options = [
		{ value: 'blake3', label: 'BLAKE3' },
		{ value: 'xxh3', label: 'XXH3' },
		{ value: 'sha256', label: 'SHA-256' }
	] as const;
</script>

<Select label="Hash algorithm" bind:value {options} />
<output data-testid="value">{value}</output>
