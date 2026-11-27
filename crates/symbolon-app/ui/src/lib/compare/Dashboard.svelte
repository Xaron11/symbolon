<!-- Spec: docs/design/components/compare-screen.md -->
<script lang="ts">
	import {
		backend,
		type CategoryName,
		type ExtensionView,
		type SizeBucketView
	} from '../api/index.ts';
	import ChartFrame from '../components/ChartFrame/ChartFrame.svelte';
	import CompositionBar from '../components/CompositionBar/CompositionBar.svelte';
	import StatTile from '../components/StatTile/StatTile.svelte';
	import { formatBytes } from '../format';
	import { CATEGORIES, isMove, sideNames } from './categories';
	import type { CompareSession } from './session.svelte';

	let { session }: { session: CompareSession } = $props();
	const api = backend();
	const count = new Intl.NumberFormat('en-US');
	let extensions = $state.raw<ExtensionView[]>([]);
	let sizes = $state.raw<SizeBucketView[]>([]);
	let refreshed = $state(0);

	const totals = $derived(
		CATEGORIES.flatMap((info) => {
			const found = session.summary?.categories.find((t) => t.category === info.name);
			return found && found.rows > 0 ? [{ info, totals: found }] : [];
		})
	);
	const selected = $derived(session.filter.categories ?? []);

	/** A tile: a category, or the only-in rows of one set of sides. */
	interface Tile {
		key: string;
		name: CategoryName;
		label: string;
		rows: number;
		detail: string;
		/** The sides the tile's rows are on exactly, for only-in tiles. */
		presence: readonly number[] | null;
	}
	const tiles = $derived<Tile[]>(
		totals.flatMap(({ info, totals: t }): Tile[] => {
			const split = info.name === 'only_in' ? (session.summary?.onlyIn ?? []) : [];
			if (split.some((s) => s.rows > 0))
				return split
					.filter((s) => s.rows > 0)
					.map((s) => ({
						key: `only_in:${s.sides.join(',')}`,
						name: info.name,
						label: `${info.label} ${sideNames(s.sides, session.sides)}`,
						rows: s.rows,
						detail: formatBytes(s.bytes),
						presence: s.sides
					}));
			const direction =
				isMove(info.name) && session.sides.length === 2
					? ` · ${session.sides.map((s) => s.label).join(' → ')}`
					: '';
			return [
				{
					key: info.name,
					name: info.name,
					label: info.label,
					rows: t.rows,
					detail: formatBytes(t.bytes) + direction,
					presence: null
				}
			];
		})
	);
	// No presence and an empty one both mean every side.
	const samePresence = (a: readonly number[] | null, b: readonly number[] | null | undefined) =>
		(a ?? []).join() === (b ?? []).join();
	const pressed = (tile: Pick<Tile, 'name' | 'presence'>) =>
		selected.length === 1 &&
		selected[0] === tile.name &&
		samePresence(tile.presence, session.filter.presence);

	$effect(() => session.changes.subscribe(() => refreshed++));

	// Charts follow the filter and every refresh of the results.
	const reads = $derived({ id: session.id, filter: session.filter, refreshed });
	$effect(() => {
		const { id, filter } = reads;
		if (id === undefined) return;
		api.compare.byExtension(id, filter).then(
			(list) => (extensions = list),
			() => undefined
		);
		api.compare.sizeHistogram(id, filter).then(
			(list) => (sizes = list),
			() => undefined
		);
	});

	// A tile picks rows by category and, for only-in tiles, by sides; other presence choices go.
	function choose(tile: Pick<Tile, 'name' | 'presence'>) {
		const on = pressed(tile);
		session.filter = {
			...session.filter,
			categories: on ? [] : [tile.name],
			presence: on || tile.presence === null ? null : [...tile.presence]
		};
	}

	const BUCKETS = ['< 1 KiB', '< 1 MiB', '< 10 MiB', '< 100 MiB', '< 1 GiB', '≥ 1 GiB'];
	const extensionLabel = (e: ExtensionView) =>
		e.extension === null ? 'Other' : e.extension === '' ? '(none)' : e.extension;
</script>

{#snippet extensionTable()}
	<table aria-label="Bytes by extension">
		<thead>
			<tr><th scope="col">Extension</th><th scope="col">Rows</th><th scope="col">Bytes</th></tr>
		</thead>
		<tbody>
			{#each extensions as e (e.extension)}
				<tr>
					<td>{extensionLabel(e)}</td><td>{count.format(e.rows)}</td><td>{formatBytes(e.bytes)}</td>
				</tr>
			{/each}
		</tbody>
	</table>
{/snippet}

{#snippet sizeTable()}
	<table aria-label="Rows by size">
		<thead>
			<tr><th scope="col">Size</th><th scope="col">Rows</th><th scope="col">Bytes</th></tr>
		</thead>
		<tbody>
			{#each sizes as bucket, i (i)}
				<tr>
					<td>{BUCKETS[i]}</td><td>{count.format(bucket.rows)}</td><td>
						{formatBytes(bucket.bytes)}
					</td>
				</tr>
			{/each}
		</tbody>
	</table>
{/snippet}

<section class="sy-dashboard" aria-label="Summary">
	<div class="tiles" role="group" aria-label="Categories">
		{#each tiles as tile (tile.key)}
			<button
				type="button"
				class="tile"
				aria-pressed={pressed(tile)}
				onclick={() => {
					choose(tile);
				}}
			>
				<StatTile
					label={tile.label}
					value={count.format(tile.rows)}
					detail={tile.detail}
					category={tile.name}
				/>
			</button>
		{/each}
	</div>
	{#if totals.length > 0}
		<CompositionBar
			label="Comparison of {session.sides.map((s) => s.label).join(' and ')}"
			segments={totals.map(({ info, totals: t }) => ({
				category: info.name,
				count: t.rows,
				bytes: t.bytes
			}))}
			onselect={(name: CategoryName) => {
				choose({ name, presence: null });
			}}
		/>
	{/if}
	<div class="charts">
		<ChartFrame
			label="Bytes by extension"
			table={extensionTable}
			config={(p: Readonly<Record<string, string>>) => ({
				type: 'bar',
				data: {
					labels: extensions.map(extensionLabel),
					datasets: [
						{
							label: 'Bytes',
							data: extensions.map((e) => e.bytes),
							backgroundColor: p['color.accent.default']
						}
					]
				},
				options: {
					plugins: { legend: { display: false } },
					scales: {
						x: { ticks: { color: p['color.text.secondary'] } },
						y: {
							ticks: { color: p['color.text.secondary'], callback: (v) => formatBytes(Number(v)) }
						}
					}
				}
			})}
		/>
		<ChartFrame
			label="Rows by size"
			table={sizeTable}
			config={(p: Readonly<Record<string, string>>) => ({
				type: 'bar',
				data: {
					labels: BUCKETS,
					datasets: [
						{
							label: 'Rows',
							data: sizes.map((b) => b.rows),
							backgroundColor: p['color.accent.default']
						}
					]
				},
				options: {
					plugins: { legend: { display: false } },
					scales: {
						x: { ticks: { color: p['color.text.secondary'] } },
						y: { ticks: { color: p['color.text.secondary'] } }
					}
				}
			})}
		/>
	</div>
</section>

<style>
	.sy-dashboard {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-16);
	}

	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(var(--sy-table-column-min), 1fr));
		gap: var(--sy-space-12);
	}

	.tile {
		padding: 0;
		border: var(--sy-border-width-strong) solid transparent;
		border-radius: var(--sy-panel-radius);
		background: none;
		color: inherit;
		font: inherit;
		text-align: start;
		cursor: pointer;
	}

	.tile[aria-pressed='true'] {
		border-color: var(--sy-color-accent-default);
	}

	.charts {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(var(--sy-scan-card-min-width), 1fr));
		gap: var(--sy-space-16);
	}
</style>
