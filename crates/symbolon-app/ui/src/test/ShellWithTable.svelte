<!-- Test harness: a DataTable inside the Shell, as a view would place it. -->
<script lang="ts">
	import DataTable from '../lib/components/DataTable/DataTable.svelte';
	import { memorySource } from '../lib/data/memory';
	import Shell from '../lib/shell/Shell.svelte';

	interface Props {
		navigate: (href: string) => void;
		selection?: ReadonlySet<number>;
	}

	let { navigate, selection = $bindable(new Set()) }: Props = $props();
	const rows = [{ name: 'b' }, { name: 'a' }];
	const columns = [
		{ id: 'name', header: 'Name', cell: (r: { name: string }) => r.name, sortable: true }
	];
</script>

<Shell pathname="/compare" {navigate}>
	<DataTable source={memorySource(rows)} {columns} label="Files" bind:selection />
</Shell>
