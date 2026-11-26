<!-- Spec: docs/design/components/compare-screen.md -->
<script lang="ts">
	import {
		backend,
		toApiError,
		type DuplicateSetView,
		type DuplicateTotalsView
	} from '../api/index.ts';
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

	async function more(side: number) {
		if (session.id === undefined) return;
		const shown = groups[side] ?? [];
		const next = await api.compare.duplicates(session.id, side, shown.length, PAGE);
		groups = groups.map((list, i) => (i === side ? [...list, ...next] : list));
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
			<p class="total">
				{total.groups === 0
					? `${label}: no duplicates`
					: `${label}: ${count.format(total.groups)} groups, ${count.format(total.files)} files, ${formatBytes(total.redundantBytes)} redundant`}
			</p>
			{#if shown.length > 0}
				<ul>
					{#each shown as set (set.digestShort)}
						<li>
							<details>
								<summary>
									<span class="sy-mono">{set.digestShort}</span>
									<span>{set.cells.length} copies</span>
									<span>{formatBytes(set.redundantBytes)} redundant</span>
								</summary>
								<ul class="paths">
									{#each set.cells as cell, i (i)}<li class="sy-mono">{cell.path}</li>{/each}
								</ul>
							</details>
						</li>
					{/each}
				</ul>
				{#if shown.length < total.groups}
					<Button
						size="sm"
						variant="secondary"
						onclick={() => {
							void more(side);
						}}
					>
						Show more
					</Button>
				{/if}
			{/if}
		</div>
	{/each}
</section>

<style>
	.sy-duplicates {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-12);
	}

	h2 {
		margin: 0;
		font-size: var(--sy-font-size-body);
		font-weight: var(--sy-font-weight-medium);
	}

	.side {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-8);
	}

	.total {
		margin: 0;
		font-weight: var(--sy-font-weight-medium);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	summary {
		display: flex;
		gap: var(--sy-space-16);
		padding: var(--sy-space-4) 0;
		cursor: pointer;
	}

	.paths {
		padding: var(--sy-space-4) 0 var(--sy-space-8) var(--sy-space-24);
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
	}
</style>
