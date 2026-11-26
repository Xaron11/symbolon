<!-- Spec: docs/design/components/scan-card.md -->
<script lang="ts">
	import { ScanSearch } from '@lucide/svelte';
	import { backend, recentScans, toApiError, type RecentScanView } from '../api/index.ts';
	import DataTable, { type Column } from '../components/DataTable/DataTable.svelte';
	import EmptyState from '../components/EmptyState/EmptyState.svelte';
	import { toasts } from '../feedback/toasts.svelte';
	import { formatAgo, formatBytes } from '../format';
	import { explain } from '../locations/status';
	import ScanCard from './ScanCard.svelte';
	import { outcomeLabel } from './progress';
	import { scans } from './scans.svelte';

	interface Props {
		/** Opens a route. */
		navigate: (href: string) => void;
	}

	let { navigate }: Props = $props();
	const api = backend();
	const scanState = scans();
	const count = new Intl.NumberFormat('en-US');
	let recentTotal = $state<number | undefined>(undefined);
	// Bumped when a scan ends, so the history is read again.
	let generation = $state(0);

	async function loadTotal() {
		recentTotal = await api.scans.recentCount().catch(() => 0);
	}

	void loadTotal();
	$effect(() =>
		scanState.onFinished(() => {
			generation++;
			void loadTotal();
		})
	);

	async function control(work: () => Promise<void>) {
		try {
			await work();
		} catch (e) {
			toasts.show(explain(toApiError(e)), 'error');
		}
	}

	const columns: Column<RecentScanView>[] = [
		{ id: 'label', header: 'Location', cell: (r: RecentScanView) => r.label },
		{
			id: 'started',
			header: 'Started',
			cell: (r: RecentScanView) => formatAgo(r.scan.startedMs),
			width: 140
		},
		{
			id: 'outcome',
			header: 'Outcome',
			cell: (r: RecentScanView) => outcomeLabel(r.scan.status),
			width: 120
		},
		{
			id: 'files',
			header: 'Files',
			cell: (r: RecentScanView) => count.format(r.scan.files),
			mono: true,
			align: 'end',
			width: 110
		},
		{
			id: 'bytes',
			header: 'Size',
			cell: (r: RecentScanView) => formatBytes(r.scan.bytes),
			mono: true,
			align: 'end',
			width: 110
		}
	];
</script>

<div class="scans">
	{#if scanState.running.length > 0}
		<section aria-label="Running scans" class="cards">
			{#each scanState.running as update (update.label)}
				<ScanCard
					{update}
					paused={scanState.isPaused(update.label)}
					onpause={() => void control(() => scanState.pause(update.label))}
					onresume={() => void control(() => scanState.resume(update.label))}
					oncancel={() => void control(() => scanState.cancel(update.label))}
				/>
			{/each}
		</section>
	{/if}

	{#if recentTotal === 0 && scanState.running.length === 0}
		<EmptyState
			title="No scans yet"
			description="Start one from Locations; it shows here while it runs and afterwards."
			icon={ScanSearch}
		>
			{#snippet actions()}<a href="/locations">Go to Locations</a>{/snippet}
		</EmptyState>
	{:else if recentTotal !== undefined && recentTotal > 0}
		<section class="recent" aria-label="History">
			{#key generation}
				<DataTable
					source={recentScans(api)}
					{columns}
					label="Recent scans"
					onopen={(row: RecentScanView) => {
						navigate(`/locations/${encodeURIComponent(row.label)}`);
					}}
				/>
			{/key}
		</section>
	{/if}
</div>

<style>
	.scans {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-16);
		height: 100%;
		min-height: 0;
	}

	.cards {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(var(--sy-scan-card-min-width), 1fr));
		gap: var(--sy-space-12);
	}

	.recent {
		height: var(--sy-pane-height);
		min-height: 0;
	}
</style>
