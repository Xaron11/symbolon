<!-- Spec: docs/design/components/compare-screen.md -->
<script lang="ts">
	import {
		backend,
		toApiError,
		type DuplicateSetView,
		type DuplicateTotalsView
	} from '../api/index.ts';
	import { ChevronRight } from '@lucide/svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import Alert from '../components/Alert/Alert.svelte';
	import Button from '../components/Button/Button.svelte';
	import { formatBytes } from '../format';
	import { explain } from '../locations/status';
	import type { CompareSession } from './session.svelte';

	let { session }: { session: CompareSession } = $props();
	const api = backend();
	const count = new Intl.NumberFormat('en-US');
	const PAGE = 100;
	const headingId = $props.id();
	let totals = $state.raw<DuplicateTotalsView[]>([]);
	let groups = $state.raw<DuplicateSetView[][]>([]);
	let error = $state<string | undefined>(undefined);
	let refreshed = $state(0);

	$effect(() => session.changes.subscribe(() => refreshed++));

	// Totals and the first page of every side, again after each verification round.
	const reads = $derived({ id: session.id, refreshed });
	$effect(() => {
		const { id } = reads;
		if (id === undefined) return;
		api.compare
			.duplicateTotals(id)
			.then(async (list) => {
				totals = list;
				groups = await Promise.all(
					list.map((_, side) => api.compare.duplicates(id, side, 0, PAGE))
				);
				error = undefined;
			})
			.catch((e: unknown) => (error = explain(toApiError(e))));
	});

	// One page read per side at a time; a page that started before a refresh is dropped.
	const reading = new SvelteSet<number>();

	async function more(side: number) {
		const id = session.id;
		if (id === undefined || reading.has(side)) return;
		reading.add(side);
		const started = refreshed;
		try {
			const next = await api.compare.duplicates(id, side, (groups[side] ?? []).length, PAGE);
			if (started === refreshed) {
				groups = groups.map((list, i) => (i === side ? [...list, ...next] : list));
			}
		} catch (e) {
			error = explain(toApiError(e));
		} finally {
			reading.delete(side);
		}
	}
</script>

<section class="sy-duplicates" aria-labelledby={headingId}>
	<h2 id={headingId}>Duplicates</h2>
	{#if error}
		<Alert tone="error" title="Could not read the duplicates">{error}</Alert>
	{/if}
	{#each totals as total, side (side)}
		{@const label = session.sides[side]?.label ?? `Side ${String(side + 1)}`}
		{@const shown = groups[side] ?? []}
		<div class="side">
			<header class="side-head">
				<h3>{label}</h3>
				<p class="total">
					{total.groups === 0
						? 'No duplicates'
						: `${count.format(total.groups)} groups · ${count.format(total.files)} files · ${formatBytes(total.redundantBytes)} redundant`}
				</p>
			</header>
			{#if shown.length > 0}
				<ul class="groups">
					{#each shown as set (set.digestShort)}
						<li>
							<details>
								<summary>
									<ChevronRight class="chevron" aria-hidden="true" />
									<span class="sy-mono digest">{set.digestShort}</span>
									<span>{set.cells.length} copies</span>
									<span class="redundant">{formatBytes(set.redundantBytes)} redundant</span>
								</summary>
								<ul class="paths">
									{#each set.cells as cell, i (i)}<li class="sy-mono">{cell.path}</li>{/each}
								</ul>
							</details>
						</li>
					{/each}
				</ul>
				{#if shown.length < total.groups}
					<div class="more">
						<Button
							size="sm"
							variant="secondary"
							onclick={() => {
								void more(side);
							}}
						>
							Show more
						</Button>
					</div>
				{/if}
			{/if}
		</div>
	{/each}
</section>

<style>
	.sy-duplicates {
		display: grid;
		gap: var(--sy-space-12);
	}

	h2 {
		margin: 0;
		font-size: var(--sy-font-size-title);
		font-weight: var(--sy-font-weight-semibold);
	}

	.side {
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-base);
	}

	.side-head {
		display: flex;
		align-items: baseline;
		gap: var(--sy-space-12);
		padding: var(--sy-space-12) var(--sy-space-16);
	}

	h3 {
		margin: 0;
		font-size: var(--sy-font-size-body);
		font-weight: var(--sy-font-weight-semibold);
	}

	.total {
		margin: 0;
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.groups > li {
		border-top: var(--sy-border-width-default) solid var(--sy-color-border-default);
	}

	summary {
		display: grid;
		grid-template-columns: var(--sy-icon-size-sm) minmax(0, 1fr) auto auto;
		gap: var(--sy-space-16);
		align-items: center;
		height: var(--sy-table-row-height);
		padding: 0 var(--sy-space-16);
		font-size: var(--sy-font-size-small);
		cursor: pointer;
		list-style: none;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	summary:hover {
		background: var(--sy-color-surface-hover);
	}

	summary :global(.chevron) {
		width: var(--sy-icon-size-sm);
		height: var(--sy-icon-size-sm);
		color: var(--sy-color-text-muted);
		transition: transform var(--sy-motion-duration-state);
	}

	details[open] summary :global(.chevron) {
		transform: rotate(90deg);
	}

	.digest {
		color: var(--sy-color-text-secondary);
	}

	.redundant {
		min-width: var(--sy-table-column-min);
		text-align: end;
	}

	.paths {
		display: grid;
		gap: var(--sy-space-4);
		padding: var(--sy-space-4) var(--sy-space-16) var(--sy-space-12)
			calc(var(--sy-space-16) * 2 + var(--sy-icon-size-sm));
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
	}

	.more {
		padding: var(--sy-space-8) var(--sy-space-16) var(--sy-space-12);
		border-top: var(--sy-border-width-default) solid var(--sy-color-border-default);
	}
</style>
