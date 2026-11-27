<!-- Spec: docs/design/components/row-detail.md -->
<script lang="ts">
	import { X } from '@lucide/svelte';
	import type { CompareRowView, SideRequest } from '../api/index.ts';
	import CategoryChip from '../components/CategoryChip/CategoryChip.svelte';
	import IconButton from '../components/IconButton/IconButton.svelte';
	import { formatBytes } from '../format';
	import { rowSummary } from './categories';

	interface Props {
		row: CompareRowView;
		sides: readonly SideRequest[];
		onclose: () => void;
	}

	let { row, sides, onclose }: Props = $props();
	const HASHED = {
		Unhashed: 'Not hashed',
		Partial: 'Partly hashed',
		Verified: 'Verified'
	} as const;
	const label = (side: number) => sides[side]?.label ?? `Side ${String(side + 1)}`;
	// Every side of the comparison, in order, so a missing file is as visible as a present one.
	const perSide = $derived(
		sides.map((_, side) => ({ side, cells: row.cells.filter((c) => c.side === side) }))
	);
</script>

<aside class="sy-row-detail" aria-label={row.display}>
	<header class="head">
		<CategoryChip category={row.category} />
		<IconButton label="Close" icon={X} variant="ghost" onclick={onclose} />
	</header>
	<h3 class="path sy-mono">{row.display}</h3>
	<p class="summary">{rowSummary(row, sides)}</p>
	{#if row.assumed}<p class="note">Equal by size and time only; not verified.</p>{/if}
	{#each perSide as { side, cells } (side)}
		<section class="side" class:missing={cells.length === 0}>
			<h4>{label(side)}</h4>
			{#if cells.length === 0}
				<p class="absent">Not on {label(side)}</p>
			{/if}
			{#each cells as cell, i (i)}
				<dl>
					<dt>Path</dt>
					<dd class="sy-mono">{cell.path}</dd>
					<dt>Size</dt>
					<dd class="sy-mono">
						{cell.kind === 'File' ? formatBytes(cell.size) : cell.kind.toLowerCase()}
					</dd>
					{#if cell.mtimeMs !== null}
						<dt>Modified</dt>
						<dd>{new Date(cell.mtimeMs).toLocaleString('en-CA')}</dd>
					{/if}
					<dt>Content</dt>
					<dd>
						{HASHED[cell.hashState]}{#if cell.digestShort}
							<span class="sy-mono digest">{cell.digestShort}</span>{/if}
					</dd>
				</dl>
			{/each}
		</section>
	{/each}
</aside>

<style>
	.sy-row-detail {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-12);
		min-width: 0;
		padding: var(--sy-space-16);
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-base);
		overflow: auto;
	}

	.head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--sy-space-8);
	}

	.path {
		margin: 0;
		overflow-wrap: anywhere;
		font-size: var(--sy-font-size-body);
		font-weight: var(--sy-font-weight-medium);
	}

	.summary {
		margin: 0;
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
	}

	.side {
		display: grid;
		gap: var(--sy-space-8);
		padding-top: var(--sy-space-12);
		border-top: var(--sy-border-width-default) solid var(--sy-color-border-default);
	}

	h4 {
		margin: 0;
		font-size: var(--sy-font-size-small);
		font-weight: var(--sy-font-weight-semibold);
	}

	dl {
		display: grid;
		grid-template-columns: max-content minmax(0, 1fr);
		gap: var(--sy-space-4) var(--sy-space-12);
		margin: 0;
		font-size: var(--sy-font-size-small);
	}

	dt {
		color: var(--sy-color-text-muted);
	}

	dd {
		margin: 0;
		overflow-wrap: anywhere;
		color: var(--sy-color-text-primary);
	}

	.digest {
		margin-left: var(--sy-space-4);
		color: var(--sy-color-text-secondary);
	}

	.absent {
		margin: 0;
		color: var(--sy-color-text-muted);
		font-size: var(--sy-font-size-small);
	}

	.note {
		margin: 0;
		color: var(--sy-status-warning-fg);
		font-size: var(--sy-font-size-small);
	}
</style>
