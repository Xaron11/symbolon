<!-- Spec: docs/design/components/composition-bar.md -->
<script lang="ts">
	import { category as info, type CategoryName } from '../../compare/categories';
	import { formatBytes } from '../../format';
	import CategoryChip from '../CategoryChip/CategoryChip.svelte';

	interface Segment {
		category: CategoryName;
		count: number;
		bytes?: number;
	}

	interface Props {
		segments: readonly Segment[];
		/** Accessible name of the bar, e.g. "Comparison of laptop and backup". */
		label: string;
		onselect?: (category: CategoryName) => void;
		/** Show the table of counts; otherwise only assistive technology reads it. */
		showTable?: boolean;
	}

	let { segments, label, onselect, showTable = false }: Props = $props();
	const shown = $derived(segments.filter((s) => s.count > 0));
	const total = $derived(shown.reduce((sum, s) => sum + s.count, 0));
	const number = new Intl.NumberFormat('en-US');
	const percent = (count: number) => (total === 0 ? '0.0' : ((count / total) * 100).toFixed(1));
	let focused = $state(0);
	// Segments can change; one of them must stay in the Tab order.
	const current = $derived(Math.min(focused, Math.max(0, shown.length - 1)));
	const buttons = $state<HTMLButtonElement[]>([]);

	function onkeydown(event: KeyboardEvent) {
		const moves: Record<string, number> = {
			ArrowRight: current + 1,
			ArrowLeft: current - 1,
			Home: 0,
			End: shown.length - 1
		};
		const next = moves[event.key];
		if (next === undefined) return;
		event.preventDefault();
		focused = Math.min(shown.length - 1, Math.max(0, next));
		buttons[focused]?.focus();
	}
</script>

<div class="sy-composition">
	<div class="bar" role="group" aria-label={label}>
		{#each shown as segment, i (segment.category)}
			{@const shownInfo = info(segment.category)}
			<button
				type="button"
				class="segment"
				tabindex={i === current ? 0 : -1}
				aria-label="{shownInfo.label}: {number.format(segment.count)} ({percent(segment.count)}%)"
				title="{shownInfo.label}: {number.format(segment.count)} ({percent(segment.count)}%)"
				style:flex-grow={segment.count}
				style:--segment-color="var(--sy-category-{shownInfo.tokenKey}-fg)"
				bind:this={buttons[i]}
				onfocus={() => (focused = i)}
				{onkeydown}
				onclick={() => onselect?.(segment.category)}
			></button>
		{/each}
	</div>
	<table class:sy-visually-hidden={!showTable}>
		<thead>
			<tr>
				<th scope="col">Category</th>
				<th scope="col">Entries</th>
				<th scope="col">Share</th>
				<th scope="col">Size</th>
			</tr>
		</thead>
		<tbody>
			{#each shown as segment (segment.category)}
				<tr>
					<td><CategoryChip category={segment.category} /></td>
					<td class="sy-mono">{number.format(segment.count)}</td>
					<td class="sy-mono">{percent(segment.count)}%</td>
					<td class="sy-mono">{segment.bytes === undefined ? '—' : formatBytes(segment.bytes)}</td>
				</tr>
			{/each}
		</tbody>
	</table>
</div>

<style>
	.sy-composition {
		display: grid;
		gap: var(--sy-space-12);
	}

	.bar {
		display: flex;
		gap: var(--sy-space-2);
		height: var(--sy-control-height-sm);
	}

	/* Every non-zero category stays visible and clickable, however small its share. */
	.segment {
		flex-basis: 0;
		min-width: var(--sy-space-4);
		padding: 0;
		border: 0;
		border-radius: var(--sy-radius-xs);
		background: var(--segment-color);
		cursor: pointer;
	}

	.segment:focus-visible {
		outline-offset: var(--sy-space-2);
	}

	table {
		border-collapse: collapse;
	}

	th {
		padding: var(--sy-space-4) var(--sy-space-8);
		border-bottom: var(--sy-border-width-default) solid var(--sy-color-border-default);
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		font-weight: var(--sy-font-weight-medium);
		text-align: start;
	}

	td {
		padding: var(--sy-space-4) var(--sy-space-8);
	}

	th:not(:first-child),
	td:not(:first-child) {
		text-align: end;
	}

	@media (pointer: coarse) {
		.bar {
			height: var(--sy-control-height-touch);
		}
	}
</style>
