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
		/** A color (CSS value) for a dot before the text, such as a category's. */
		marker?: (row: R) => string | undefined;
	}
</script>

<script lang="ts" generics="Row">
	import { SvelteSet } from 'svelte/reactivity';
	import { component } from '../../design/generated/tokens';
	import type { DataSource, SortSpec } from '../../data/source';
	import { createRowWindow, revealScroll } from '../../data/window.svelte';
	import { createDispatcher } from '../../keyboard/dispatcher';
	import { globalActions } from '../../keyboard/global';
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
	const headerHeight = parseFloat(component['table.header.height']);
	// svelte-ignore state_referenced_locally
	const rows = createRowWindow(source, { rowHeight });
	const id = $props.id();
	let active = $state(0);
	let activeColumn = $state(0);
	// The cell cursor only matters once someone moves between columns (h and l).
	let columnCursor = $state(false);
	// Row 0 is active from the start; it is only marked once someone chose a row.
	let chosen = $state(false);
	let grid = $state<HTMLDivElement>();
	let scroller = $state<HTMLDivElement>();
	let height = $state(0);

	// Header and rows share one track list and one least width, so columns line up and a long
	// value is cut with an ellipsis instead of widening its row.
	const flexMin = 'var(--sy-table-column-min)';
	const template = $derived(
		columns
			.map((c) => (c.width === undefined ? `minmax(${flexMin}, 1fr)` : `${String(c.width)}px`))
			.join(' ')
	);
	const minWidth = $derived(
		`calc(${String(columns.reduce((sum, c) => sum + (c.width ?? 0), 0))}px + ${String(
			columns.filter((c) => c.width === undefined).length
		)} * ${flexMin})`
	);
	const total = $derived(rows.count ?? 0);
	const visibleRows = $derived(Math.max(1, Math.floor(height / rowHeight)));
	const activeLoaded = $derived(rows.rows[active - rows.start] !== undefined);

	$effect(() => {
		rows.setViewport(scroller?.scrollTop ?? 0, height);
	});

	// A source that changes (a comparison being verified) is re-read where it is shown.
	// Selection is by position, which the new rows give to other rows.
	$effect(() =>
		source.subscribe?.(() => {
			rows.refresh();
			selection = new SvelteSet();
		})
	);
	$effect(() => {
		if (total > 0 && active > total - 1) active = total - 1;
	});

	function moveTo(index: number) {
		if (total === 0) return;
		active = Math.min(total - 1, Math.max(0, index));
		if (scroller) {
			scroller.scrollTop = revealScroll({
				index: active,
				scrollTop: scroller.scrollTop,
				height,
				rowHeight,
				headerHeight
			});
			rows.setViewport(scroller.scrollTop, height);
		}
	}

	function onAction(action: Action) {
		if (action.startsWith('list.')) chosen = true;
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
				columnCursor = true;
				break;
			case 'list.expand':
				activeColumn = Math.min(columns.length - 1, activeColumn + 1);
				columnCursor = true;
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
			case 'list.sort': {
				const column = columns[activeColumn];
				if (column?.sortable) toggleSort(column.id);
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
				forwardGlobal?.(action);
				break;
		}
	}

	const forwardGlobal = globalActions();
	const dispatch = createDispatcher(
		KEYMAP,
		() => (forwardGlobal ? ['global', 'list'] : ['list']),
		onAction
	);

	function toggleSort(column: string) {
		const current = rows.sort;
		const next: SortSpec | undefined =
			current?.column !== column
				? { column, direction: 'asc' }
				: current.direction === 'asc'
					? { column, direction: 'desc' }
					: undefined;
		rows.setSort(next);
		// Selection is by position, which a new order gives to other rows.
		selection = new SvelteSet();
	}

	function ariaSort(column: Column<Row>) {
		if (!column.sortable) return undefined;
		if (rows.sort?.column !== column.id) return 'none';
		return rows.sort.direction === 'asc' ? 'ascending' : 'descending';
	}
</script>

<div
	bind:this={grid}
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
		<div
			class="header"
			role="row"
			aria-rowindex="1"
			style:grid-template-columns={template}
			style:min-width={minWidth}
		>
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
								grid?.focus();
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
				<!-- Rows are reached through the grid (aria-activedescendant), not focused themselves;
				     clicking one makes it active and keeps focus on the grid. -->
				<!-- svelte-ignore a11y_click_events_have_key_events, a11y_interactive_supports_focus -->
				<div
					class="row"
					class:active={index === active}
					class:marked={chosen && index === active}
					role="row"
					aria-rowindex={index + 2}
					aria-selected={selection.has(index)}
					style:transform="translateY({String(index * rowHeight)}px)"
					style:grid-template-columns={template}
					style:min-width={minWidth}
					onclick={() => {
						active = index;
						chosen = true;
						columnCursor = false;
						grid?.focus();
					}}
					ondblclick={() => {
						if (row !== undefined) onopen?.(row, index);
					}}
				>
					{#each columns as column, c (column.id)}
						{@const text = row === undefined ? '' : column.cell(row)}
						{@const marker = row === undefined ? undefined : column.marker?.(row)}
						<div
							id="{id}-r{index}-c{c}"
							class="cell truncate"
							class:sy-mono={column.mono}
							class:end={column.align === 'end'}
							class:current={columnCursor && index === active && c === activeColumn}
							role="gridcell"
							title={text}
						>
							{#if marker}<span class="marker" style:background={marker} aria-hidden="true"
								></span>{/if}{text}
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
	}

	.header {
		position: sticky;
		top: 0;
		z-index: 1;
		height: var(--sy-table-header-height);
		/* Matches the rows' accent-bar space, so header and cells line up. */
		border-left: var(--sy-border-width-strong) solid transparent;
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

	/* The chosen row stays gray when focus goes elsewhere (the details, another window); the
	   accent bar shows where keys act. Clicks and keys mark it alike. */
	.row[aria-selected='true'],
	.row.marked,
	.sy-table:focus .row.active {
		background: var(--sy-color-surface-selected);
	}

	.sy-table:focus .row.active {
		border-left-color: var(--sy-color-accent-default);
	}

	.cell {
		display: flex;
		min-width: 0;
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

	.marker {
		display: inline-block;
		width: var(--sy-space-8);
		height: var(--sy-space-8);
		margin-right: var(--sy-space-8);
		border-radius: var(--sy-radius-full);
		vertical-align: middle;
	}

	.end {
		justify-content: flex-end;
		text-align: end;
	}

	.sy-table:focus .current {
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
