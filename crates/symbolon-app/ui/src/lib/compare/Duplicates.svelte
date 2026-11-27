<!-- Spec: docs/design/components/compare-screen.md -->
<script lang="ts">
	import { Copy, File } from '@lucide/svelte';
	import {
		backend,
		duplicateGroups,
		toApiError,
		type DuplicateNode,
		type DuplicateTotalsView
	} from '../api/index.ts';
	import Alert from '../components/Alert/Alert.svelte';
	import TreeView from '../components/TreeView/TreeView.svelte';
	import { formatBytes } from '../format';
	import { explain } from '../locations/status';
	import type { CompareSession } from './session.svelte';

	let { session }: { session: CompareSession } = $props();
	const api = backend();
	const count = new Intl.NumberFormat('en-US');
	const headingId = $props.id();
	let totals = $state.raw<DuplicateTotalsView[]>([]);
	let error = $state<string | undefined>(undefined);
	let refreshed = $state(0);

	$effect(() => session.changes.subscribe(() => refreshed++));

	// Totals again after each verification round; the trees re-read their groups themselves.
	const reads = $derived({ id: session.id, refreshed });
	$effect(() => {
		const { id } = reads;
		if (id === undefined) return;
		api.compare.duplicateTotals(id).then(
			(list) => {
				totals = list;
				error = undefined;
			},
			(e: unknown) => (error = explain(toApiError(e)))
		);
	});

	// A group reads as its first copy; the rest are one level down.
	const label = (node: DuplicateNode) =>
		node.kind === 'group' ? (node.set.cells[0]?.path ?? node.set.digestShort) : node.path;
	const aside = (node: DuplicateNode) =>
		node.kind === 'group'
			? `${count.format(node.set.cells.length)} copies · ${formatBytes(node.set.redundantBytes)} redundant`
			: undefined;
</script>

<section class="sy-duplicates" aria-labelledby={headingId}>
	<h2 id={headingId}>Duplicates</h2>
	{#if error}
		<Alert tone="error" title="Could not read the duplicates">{error}</Alert>
	{/if}
	{#each totals as total, side (side)}
		{@const name = session.sides[side]?.label ?? `Side ${String(side + 1)}`}
		<div class="side">
			<header class="side-head">
				<h3>{name}</h3>
				<p class="total">
					{total.groups === 0
						? 'No duplicates'
						: `${count.format(total.groups)} groups · ${count.format(total.files)} files · ${formatBytes(total.redundantBytes)} redundant`}
				</p>
			</header>
			{#if total.groups > 0 && session.id !== undefined}
				<div class="pane">
					<TreeView
						source={duplicateGroups(api, session.id, side, session.changes)}
						name="Duplicates in {name}"
						{label}
						{aside}
						icon={(node: DuplicateNode) => (node.kind === 'group' ? Copy : File)}
						fit
					/>
				</div>
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
		display: grid;
		gap: var(--sy-space-8);
	}

	.side-head {
		display: flex;
		align-items: baseline;
		gap: var(--sy-space-12);
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

	.pane {
		display: flex;
		flex-direction: column;
		max-height: var(--sy-pane-height);
	}
</style>
