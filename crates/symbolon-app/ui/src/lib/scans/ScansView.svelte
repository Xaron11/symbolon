<!-- Spec: docs/design/components/scan-card.md -->
<script lang="ts">
	import { ChevronDown, ScanSearch } from '@lucide/svelte';
	import {
		backend,
		recentScans,
		toApiError,
		type LocationView,
		type RecentScanView
	} from '../api/index.ts';
	import Alert from '../components/Alert/Alert.svelte';
	import DataTable, { type Column } from '../components/DataTable/DataTable.svelte';
	import DropdownMenu from '../components/DropdownMenu/DropdownMenu.svelte';
	import EmptyState from '../components/EmptyState/EmptyState.svelte';
	import { toasts } from '../feedback/toasts.svelte';
	import { formatAgo, formatBytes } from '../format';
	import { explain, scanBlocked } from '../locations/status';
	import ScanCard from './ScanCard.svelte';
	import { outcomeColor, outcomeLabel, scanDuration } from './progress';
	import { scans } from './scans.svelte';

	interface Props {
		/** Opens a route. */
		navigate: (href: string) => void;
	}

	let { navigate }: Props = $props();
	const api = backend();
	const scanState = scans();
	const count = new Intl.NumberFormat('en-US');
	const headings = $props.id();
	let recentTotal = $state<number | undefined>(undefined);
	let loadError = $state<string | undefined>(undefined);
	let locations = $state<LocationView[]>([]);
	api.locations.list().then(
		(list) => (locations = list),
		() => undefined
	);
	const startable = $derived(
		locations.map((l) => ({
			label: l.label,
			disabled:
				scanBlocked(l.status) !== undefined || scanState.running.some((u) => u.label === l.label),
			onSelect: () => void control(() => scanState.start(l.label))
		}))
	);
	// Bumped when a scan ends, so the history is read again.
	let generation = $state(0);

	async function loadTotal() {
		try {
			recentTotal = await api.scans.recentCount();
			loadError = undefined;
		} catch (e) {
			loadError = explain(toApiError(e));
		}
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

	// A running scan's counts are only known when it ends; its card shows them meanwhile.
	const whenDone = (r: RecentScanView, value: string) =>
		r.scan.status === 'Running' ? '—' : value;
	const columns: Column<RecentScanView>[] = [
		{
			id: 'label',
			header: 'Location',
			cell: (r: RecentScanView) => r.label,
			badge: (r: RecentScanView) => (r.latest ? 'Latest' : undefined)
		},
		{
			id: 'started',
			header: 'Started',
			cell: (r: RecentScanView) => formatAgo(r.scan.startedMs),
			width: 140
		},
		{
			id: 'duration',
			header: 'Took',
			cell: (r: RecentScanView) => scanDuration(r.scan) ?? '—',
			width: 100
		},
		{
			id: 'outcome',
			header: 'Outcome',
			cell: (r: RecentScanView) => outcomeLabel(r.scan.status),
			marker: (r: RecentScanView) => outcomeColor(r.scan.status),
			width: 120
		},
		{
			id: 'files',
			header: 'Files',
			cell: (r: RecentScanView) => whenDone(r, count.format(r.scan.files)),
			mono: true,
			align: 'end',
			width: 100
		},
		{
			id: 'bytes',
			header: 'Size',
			cell: (r: RecentScanView) => whenDone(r, formatBytes(r.scan.bytes)),
			mono: true,
			align: 'end',
			width: 100
		},
		{
			id: 'warnings',
			header: 'Warnings',
			cell: (r: RecentScanView) => whenDone(r, count.format(r.scan.warnings)),
			mono: true,
			align: 'end',
			width: 90
		}
	];
</script>

<div class="scans">
	{#if startable.length > 0}
		<div class="toolbar">
			<p class="summary">
				{#if recentTotal !== undefined && recentTotal > 0}
					{count.format(recentTotal)}
					{recentTotal === 1 ? 'scan' : 'scans'} of {count.format(locations.length)}
					{locations.length === 1 ? 'location' : 'locations'}
				{/if}
			</p>
			<DropdownMenu label="Scan location" items={startable} align="end">
				{#snippet trigger()}Scan location<ChevronDown aria-hidden="true" />{/snippet}
			</DropdownMenu>
		</div>
	{/if}
	{#if scanState.running.length > 0}
		<section aria-labelledby="{headings}-running" class="running">
			<h2 id="{headings}-running">Running</h2>
			<div class="cards">
				{#each scanState.running as update (update.label)}
					<ScanCard
						{update}
						paused={scanState.isPaused(update.label)}
						onpause={() => void control(() => scanState.pause(update.label))}
						onresume={() => void control(() => scanState.resume(update.label))}
						oncancel={() => void control(() => scanState.cancel(update.label))}
					/>
				{/each}
			</div>
		</section>
	{/if}

	{#if loadError}
		<Alert tone="error" title="Could not read the scan history">{loadError}</Alert>
	{:else if recentTotal === 0 && scanState.running.length === 0}
		<EmptyState
			title="No scans yet"
			description="Start one from Locations; it shows here while it runs and afterwards."
			icon={ScanSearch}
		>
			{#snippet actions()}<a href="/locations">Go to Locations</a>{/snippet}
		</EmptyState>
	{:else if recentTotal !== undefined && recentTotal > 0}
		<section class="recent" aria-labelledby="{headings}-recent">
			<h2 id="{headings}-recent">Recent scans</h2>
			{#key generation}
				<DataTable
					source={recentScans(api)}
					{columns}
					label="Recent scans"
					empty="No scans yet."
					fit
					openOnClick
					onopen={(row: RecentScanView) => {
						navigate(`/scans/${String(row.scan.scanId)}`);
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

	.toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--sy-space-12);
	}

	.summary {
		margin: 0;
		color: var(--sy-color-text-secondary);
	}

	.cards {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(var(--sy-scan-card-min-width), 1fr));
		gap: var(--sy-space-12);
	}

	.running,
	.recent {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-8);
	}

	.recent {
		height: var(--sy-pane-height);
		min-height: 0;
	}

	h2 {
		margin: 0;
		font-size: var(--sy-font-size-body);
		font-weight: var(--sy-font-weight-medium);
	}
</style>
