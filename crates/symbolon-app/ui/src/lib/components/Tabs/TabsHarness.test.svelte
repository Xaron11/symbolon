<script lang="ts">
	import Tabs from './Tabs.svelte';

	let value = $state<'summary' | 'files'>('summary');
</script>

{#snippet summary()}Summary panel{/snippet}
{#snippet files()}Files panel{/snippet}
<Tabs
	label="Comparison"
	bind:value
	tabs={[
		{ value: 'summary', label: 'Summary', content: summary },
		{ value: 'files', label: 'Files', content: files }
	]}
/>
