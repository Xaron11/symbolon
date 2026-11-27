<!-- Spec: docs/design/components/row-detail.md -->
<script lang="ts">
	import { X } from '@lucide/svelte';
	import type { CompareRowView, SideRequest } from '../api/index.ts';
	import CategoryChip from '../components/CategoryChip/CategoryChip.svelte';
	import IconButton from '../components/IconButton/IconButton.svelte';
	import { formatBytes } from '../format';

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
</script>

<aside class="sy-row-detail" aria-label={row.display}>
	<header class="head">
		<h3 class="sy-mono">{row.display}</h3>
		<IconButton label="Close" icon={X} variant="ghost" onclick={onclose} />
	</header>
	<CategoryChip category={row.category} />
	{#if row.assumed}<p class="note">Equal by size and time only; not verified.</p>{/if}
	<dl>
		{#each row.cells as cell, i (i)}
			<div class="cell">
				<dt>{sides[cell.side]?.label ?? `Side ${String(cell.side + 1)}`}</dt>
				<dd class="sy-mono">{cell.path}</dd>
				<dd>
					{cell.kind === 'File' ? formatBytes(cell.size) : cell.kind.toLowerCase()}
					{#if cell.mtimeMs !== null}· {new Date(cell.mtimeMs).toLocaleString('en-CA')}{/if}
				</dd>
				<dd>
					{HASHED[cell.hashState]}{#if cell.digestShort}
						· <span class="sy-mono">{cell.digestShort}</span>{/if}
				</dd>
			</div>
		{/each}
	</dl>
</aside>

<style>
	.sy-row-detail {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-8);
		padding: var(--sy-space-12);
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-raised);
		overflow: auto;
	}

	.head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--sy-space-8);
	}

	h3 {
		margin: 0;
		overflow-wrap: anywhere;
		font-size: var(--sy-font-size-body);
	}

	dl {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-12);
		margin: 0;
	}

	dt {
		font-weight: var(--sy-font-weight-medium);
	}

	dd {
		margin: 0;
		overflow-wrap: anywhere;
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
	}

	.note {
		margin: 0;
		color: var(--sy-status-warning-fg);
		font-size: var(--sy-font-size-small);
	}
</style>
