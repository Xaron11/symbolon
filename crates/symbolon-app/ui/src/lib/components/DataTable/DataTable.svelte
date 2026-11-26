<!-- Spec: docs/design/components/data-table.md -->
<script lang="ts" module>
	/** A column: how to show one value of a row. */
	export interface Column<R> {
		id: string;
		header: string;
		cell: (row: R) => string;
		/** Paths, hashes, sizes: monospace with tabular figures. */
		mono?: boolean;
		align?: 'start' | 'end';
		/** Fixed width in pixels; columns without one share the rest. */
		width?: number;
		sortable?: boolean;
	}
</script>

<script lang="ts" generics="Row">
	import { SvelteSet } from 'svelte/reactivity';
	import { component } from '../../design/generated/tokens';
	import type { DataSource, SortSpec } from '../../data/source';
	import { createRowWindow } from '../../data/window.svelte';
	import { createDispatcher } from '../../keyboard/dispatcher';
	import { KEYMAP, type Action } from '../../keyboard/keymap';

	interface Props {
		/** Read once; give the table a new `{#key}` to show another source. */
		source: DataSource<Row>;
		columns: readonly Column<Row>[];
		/** Accessible name of the grid. */
		label: string;
		/** Indices of selected rows. */
		selection?: ReadonlySet<number>;
		onopen?: (row: Row, index: number) => void;
	}

	let { source, columns, label, selection = $bindable(new Set()), onopen }: Props = $props();

	const rowHeight = parseFloat(component['table.row.height']);
	// svelte-ignore state_referenced_locally
	const rows = createRowWindow(source, { rowHeight });
	const id = $props.id();
	let active = $state(0);
	let activeColumn = $state(0);
	let scroller = $state<HTMLDivElement>();
	let height = $state(0);

	const template = $derived(
		columns
			.map((c) => (c.width === undefined ? 'minmax(0, 1fr)' : `${String(c.width)}px`))
			.join(' ')
	);
	const total = $derived(rows.count ?? 0);
	const visibleRows = $derived(Math.max(1, Math.floor(height / rowHeight)));
	const activeLoaded = $derived(rows.rows[active - rows.start] !== undefined);

	$effect(() => {
		rows.setViewport(scroller?.scrollTop ?? 0, height);
	});

	function moveTo(index: number) {
		if (total === 0) return;
		active = Math.min(total - 1, Math.max(0, index));
		const top = active * rowHeight;
		if (scroller) {
			if (top < scroller.scrollTop) scroller.scrollTop = top;
			else if (top + rowHeight > scroller.scrollTop + height)
				scroller.scrollTop = top + rowHeight - height;
			rows.setViewport(scroller.scrollTop, height);
		}
	}

	function onAction(action: Action) {
		switch (action) {
			case 'list.down':
				moveTo(active + 1);
				break;
			case 'list.up':
				moveTo(active - 1);
				break;
			case 'list.top':
				moveTo(0);
				break;
			case 'list.bottom':
				moveTo(total - 1);
				break;
			case 'list.halfPageDown':
				moveTo(active + Math.ceil(visibleRows / 2));
				break;
			case 'list.halfPageUp':
				moveTo(active - Math.ceil(visibleRows / 2));
				break;
			case 'list.pageDown':
				moveTo(active + visibleRows);
				break;
			case 'list.pageUp':
				moveTo(active - visibleRows);
				break;
			case 'list.collapse':
				activeColumn = Math.max(0, activeColumn - 1);
				break;
			case 'list.expand':
				activeColumn = Math.min(columns.length - 1, activeColumn + 1);
				break;
			case 'list.toggleSelect': {
				const next = new SvelteSet(selection);
				if (next.has(active)) next.delete(active);
				else next.add(active);
				selection = next;
				break;
			}
			case 'list.open': {
				const row = rows.rows[active - rows.start];
				if (row !== undefined) onopen?.(row, active);
				break;
			}
			case 'list.filter':
			case 'list.nextMatch':
			case 'list.prevMatch':
				// Filtering and matches belong to the view that owns the table.
				break;
			case 'escape':
			case 'help.open':
			case 'palette.open':
			case 'nav.locations':
			case 'nav.scans':
			case 'nav.compare':
			case 'sidebar.toggle':
				// Global actions; only list bindings are active here.
				break;
		}
	}

	const dispatch = createDispatcher(KEYMAP, () => ['list'], onAction);

	function toggleSort(column: string) {
		const current = rows.sort;
		const next: SortSpec | undefined =
			current?.column !== column
				? { column, direction: 'asc' }
				: current.direction === 'asc'
					? { column, direction: 'desc' }
					: undefined;
		rows.setSort(next);
	}

	function ariaSort(column: Column<Row>) {
		if (!column.sortable) return undefined;
		if (rows.sort?.column !== column.id) return 'none';
		return rows.sort.direction === 'asc' ? 'ascending' : 'descending';
	}
</script>

<div
	class="sy-table"
	role="grid"
	aria-label={label}
	aria-rowcount={total + 1}
	aria-colcount={columns.length}
	aria-multiselectable="true"
	aria-activedescendant={activeLoaded
		? `${id}-r${String(active)}-c${String(activeColumn)}`
		: undefined}
	tabindex="0"
	onkeydown={(event) => {
		dispatch(event);
	}}
>
	<div
		class="body"
		bind:this={scroller}
		bind:clientHeight={height}
		onscroll={() => {
			requestAnimationFrame(() => {
				rows.setViewport(scroller?.scrollTop ?? 0, height);
			});
		}}
	>
		<div class="header" role="row" aria-rowindex="1" style:grid-template-columns={template}>
			{#each columns as column (column.id)}
				<div
					class="cell"
					class:end={column.align === 'end'}
					role="columnheader"
					aria-sort={ariaSort(column)}
				>
					{#if column.sortable}
						<button
							type="button"
							tabindex="-1"
							onclick={() => {
								toggleSort(column.id);
							}}
						>
							{column.header}
						</button>
					{:else}
						{column.header}
					{/if}
				</div>
			{/each}
		</div>
		<div class="spacer" role="presentation" style:height="{String(total * rowHeight)}px">
			{#each rows.rows as row, i (rows.start + i)}
				{@const index = rows.start + i}
				<!-- Clicking only moves the active cell; the grid's keys do the same for keyboards. -->
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<div
					class="row"
					class:active={index === active}
					role="row"
					tabindex="-1"
					aria-rowindex={index + 2}
					aria-selected={selection.has(index)}
					style:transform="translateY({String(index * rowHeight)}px)"
					style:grid-template-columns={template}
					onclick={() => (active = index)}
				>
					{#each columns as column, c (column.id)}
						{@const text = row === undefined ? '' : column.cell(row)}
						<div
							id="{id}-r{index}-c{c}"
							class="cell truncate"
							class:sy-mono={column.mono}
							class:end={column.align === 'end'}
							class:current={index === active && c === activeColumn}
							role="gridcell"
							title={text}
						>
							{text}
						</div>
					{/each}
				</div>
			{/each}
		</div>
	</div>
	{#if rows.error}
		<p class="error" role="alert">
			Could not load rows: {rows.error.message}
			<button
				type="button"
				onclick={() => {
					rows.retry();
				}}
			>
				Retry
			</button>
		</p>
	{/if}
</div>

<style>
	.sy-table {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-base);
		outline-offset: calc(var(--sy-focus-ring-offset) * -1);
	}

	.body {
		flex: 1;
		min-height: 0;
		overflow: auto;
	}

	.header,
	.row {
		display: grid;
		min-width: max-content;
	}

	.header {
		position: sticky;
		top: 0;
		z-index: 1;
		height: var(--sy-table-header-height);
		border-bottom: var(--sy-border-width-default) solid var(--sy-color-border-default);
		background: var(--sy-color-surface-sunken);
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		font-weight: var(--sy-font-weight-medium);
	}

	.header button {
		padding: 0;
		border: 0;
		background: transparent;
		color: inherit;
		font: inherit;
		cursor: pointer;
	}

	.spacer {
		position: relative;
	}

	.row {
		position: absolute;
		top: 0;
		right: 0;
		left: 0;
		height: var(--sy-table-row-height);
		border-left: var(--sy-border-width-strong) solid transparent;
	}

	.row:hover {
		background: var(--sy-color-surface-hover);
	}

	.row[aria-selected='true'] {
		background: var(--sy-color-surface-selected);
	}

	.sy-table:focus-visible .row.active {
		border-left-color: var(--sy-color-accent-default);
	}

	.cell {
		display: flex;
		align-items: center;
		padding: 0 var(--sy-space-8);
	}

	.truncate {
		display: block;
		overflow: hidden;
		line-height: var(--sy-table-row-height);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.end {
		justify-content: flex-end;
		text-align: end;
	}

	.sy-table:focus-visible .current {
		outline: var(--sy-border-width-default) solid var(--sy-color-focus);
		outline-offset: calc(var(--sy-border-width-default) * -1);
	}

	.error {
		display: flex;
		align-items: center;
		gap: var(--sy-space-8);
		margin: 0;
		padding: var(--sy-space-8);
		color: var(--sy-status-error-fg);
	}
</style>
