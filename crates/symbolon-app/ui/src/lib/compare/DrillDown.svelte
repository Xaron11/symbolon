<!-- Spec: docs/design/components/drill-down.md -->
<script lang="ts">
	import { File, Folder } from '@lucide/svelte';
	import {
		backend,
		comparisonRows,
		comparisonTree,
		type CompareRowView,
		type TreeNodeView
	} from '../api/index.ts';
	import DataTable, { type Column } from '../components/DataTable/DataTable.svelte';
	import TreeView from '../components/TreeView/TreeView.svelte';
	import { formatBytes } from '../format';
	import { rowPath, rowSummary } from './categories';
	import FilterChips from './FilterChips.svelte';
	import RowDetail from './RowDetail.svelte';
	import type { CompareSession } from './session.svelte';

	let { session }: { session: CompareSession } = $props();
	const api = backend();
	const count = new Intl.NumberFormat('en-US');
	let view = $state<'list' | 'tree'>('list');
	let selected = $state<CompareRowView | undefined>(undefined);
	let pane = $state<HTMLDivElement>();

	/** Closes the row panel and gives focus back to the list or tree it was opened from. */
	function closeDetail() {
		selected = undefined;
		pane?.querySelector<HTMLElement>('[role="grid"], [role="tree"]')?.focus();
	}
	// A new filter means new sources; the key rebuilds the table or tree.
	const filterKey = $derived(JSON.stringify(session.filter));

	const sizeOn = (row: CompareRowView, side: number) => {
		const cell = row.cells.find((c) => c.side === side);
		if (!cell) return '—';
		return cell.kind === 'File' ? formatBytes(cell.size) : cell.kind.toLowerCase();
	};
	const modified = (row: CompareRowView) => {
		const latest = Math.max(...row.cells.map((c) => c.mtimeMs ?? 0));
		return latest > 0 ? new Date(latest).toLocaleDateString('en-CA') : '';
	};

	const columns = $derived<Column<CompareRowView>[]>([
		{
			id: 'path',
			header: 'Path',
			cell: rowPath,
			mono: true,
			sortable: true
		},
		{
			id: 'category',
			header: 'Category',
			cell: (r: CompareRowView) => rowSummary(r, session.sides),
			width: 200
		},
		{
			id: 'size',
			header: 'Size',
			cell: (r: CompareRowView) => (r.size === null ? '' : formatBytes(r.size)),
			mono: true,
			align: 'end',
			width: 110,
			sortable: true
		},
		...session.sides.map((side, i) => ({
			id: `side${String(i)}`,
			header: `On ${side.label}`,
			cell: (r: CompareRowView) => sizeOn(r, i),
			mono: true,
			align: 'end' as const,
			width: 120
		})),
		{ id: 'modified', header: 'Modified', cell: modified, mono: true, width: 120 }
	]);

	const nodeLabel = (node: TreeNodeView) =>
		node.folder
			? `${node.name} — ${count.format(node.matchingRows)} · ${formatBytes(node.matchingBytes)}`
			: node.row
				? `${node.name} — ${rowSummary(node.row, session.sides)}`
				: node.name;
</script>

<section class="sy-drill-down" aria-label="Rows">
	<FilterChips {session} />
	<div class="switch" role="radiogroup" aria-label="View">
		{#each [{ value: 'list', label: 'List' }, { value: 'tree', label: 'Tree' }] as const as option (option.value)}
			<button
				type="button"
				role="radio"
				aria-checked={view === option.value}
				onclick={() => {
					view = option.value;
				}}
			>
				{option.label}
			</button>
		{/each}
	</div>
	<!-- Escape closes the panel before the list or tree sees it. -->
	<div
		class="body"
		class:with-detail={selected !== undefined}
		role="presentation"
		onkeydowncapture={(event) => {
			if (event.key === 'Escape' && selected) {
				closeDetail();
				event.preventDefault();
				event.stopPropagation();
			}
		}}
	>
		<div class="pane" bind:this={pane}>
			{#if session.id !== undefined}
				{#key `${view}:${filterKey}`}
					{#if view === 'list'}
						<DataTable
							source={comparisonRows(api, session.id, session.filter, session.changes)}
							{columns}
							label="Rows"
							onopen={(row: CompareRowView) => (selected = row)}
						/>
					{:else}
						<TreeView
							source={comparisonTree(api, session.id, session.filter, session.changes)}
							name="Rows by folder"
							label={nodeLabel}
							icon={(node: TreeNodeView) => (node.folder ? Folder : File)}
							onopen={(node: TreeNodeView) => {
								if (node.row) selected = node.row;
							}}
						/>
					{/if}
				{/key}
			{/if}
		</div>
		{#if selected}
			<RowDetail row={selected} sides={session.sides} onclose={closeDetail} />
		{/if}
	</div>
</section>

<style>
	.sy-drill-down {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-12);
	}

	.switch {
		display: flex;
		gap: var(--sy-space-4);
	}

	.switch button {
		padding: var(--sy-space-4) var(--sy-space-12);
		border: var(--sy-border-width-default) solid var(--sy-color-border-strong);
		border-radius: var(--sy-control-radius);
		background: var(--sy-color-surface-base);
		color: var(--sy-color-text-primary);
		font: inherit;
		cursor: pointer;
	}

	.switch button[aria-checked='true'] {
		border-color: var(--sy-color-accent-default);
		background: var(--sy-color-surface-selected);
	}

	.body {
		display: grid;
		grid-template-columns: 1fr;
		gap: var(--sy-space-12);
	}

	.body.with-detail {
		grid-template-columns: 2fr 1fr;
	}

	.pane {
		height: var(--sy-pane-height);
		min-height: 0;
	}
</style>
