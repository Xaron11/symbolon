<!-- Spec: docs/design/components/scan-detail.md -->
<script lang="ts">
	import {
		backend,
		scanWarnings,
		toApiError,
		type RecentScanView,
		type WarningView
	} from '../api/index.ts';
	import Alert from '../components/Alert/Alert.svelte';
	import Badge from '../components/Badge/Badge.svelte';
	import Breadcrumb from '../components/Breadcrumb/Breadcrumb.svelte';
	import Button from '../components/Button/Button.svelte';
	import DataTable, { type Column } from '../components/DataTable/DataTable.svelte';
	import EmptyState from '../components/EmptyState/EmptyState.svelte';
	import Spinner from '../components/Spinner/Spinner.svelte';
	import StatTile from '../components/StatTile/StatTile.svelte';
	import { toasts } from '../feedback/toasts.svelte';
	import { formatBytes } from '../format';
	import { explain } from '../locations/status';
	import { outcomeLabel, outcomeTone, scanDuration } from './progress';
	import { scans } from './scans.svelte';

	interface Props {
		scanId: number;
		/** Opens a route, e.g. Scans when the location is already being scanned. */
		navigate: (href: string) => void;
	}

	let { scanId, navigate }: Props = $props();
	const api = backend();
	const scanState = scans();
	const count = new Intl.NumberFormat('en-US');
	// null: there is no such scan.
	let found = $state<RecentScanView | null | undefined>(undefined);
	let loadError = $state<string | undefined>(undefined);

	$effect(() => {
		if (!Number.isSafeInteger(scanId)) {
			found = null;
			return;
		}
		api.scans.get(scanId).then(
			(scan) => (found = scan ?? null),
			(e: unknown) => (loadError = explain(toApiError(e)))
		);
	});

	const when = (ms: number) =>
		new Date(ms).toLocaleString('en-GB', { dateStyle: 'medium', timeStyle: 'short' });
	const running = $derived(found ? scanState.running.some((u) => u.label === found?.label) : false);

	async function scanAgain(label: string) {
		try {
			await scanState.start(label);
			navigate('/scans');
		} catch (e) {
			const error = toApiError(e);
			if (error.kind === 'AlreadyScanning') navigate('/scans');
			else toasts.show(explain(error), 'error');
		}
	}

	const warningColumns: Column<WarningView>[] = [
		{
			id: 'path',
			header: 'Path',
			cell: (w: WarningView) => w.path ?? '(the location)',
			mono: true
		},
		{ id: 'kind', header: 'Kind', cell: (w: WarningView) => w.kind, width: 140 },
		{ id: 'message', header: 'Message', cell: (w: WarningView) => w.message }
	];
</script>

{#if loadError}
	<Alert tone="error" title="Could not read the scan">{loadError}</Alert>
{:else if found === undefined}
	<Spinner label="Loading the scan" />
{:else if found === null}
	<EmptyState title="This scan no longer exists.">
		{#snippet actions()}<a href="/scans">Back to Scans</a>{/snippet}
	</EmptyState>
{:else}
	{@const scan = found.scan}
	{@const took = scanDuration(scan)}
	<div class="sy-scan-detail">
		<Breadcrumb
			trail={[{ label: 'Scans', href: '/scans' }]}
			current="{found.label}, {when(scan.startedMs)}"
		/>
		<header class="head">
			<div class="title">
				<div class="name">
					<h2>{found.label}</h2>
					<Badge tone={outcomeTone(scan.status)}>{outcomeLabel(scan.status)}</Badge>
					{#if found.latest}<Badge tone="info">Latest scan</Badge>{/if}
				</div>
				<p class="meta">
					Started {when(scan.startedMs)}{#if took}, took {took}{/if}
				</p>
			</div>
			<div class="actions">
				<Button href="/locations/{encodeURIComponent(found.label)}">Open {found.label}</Button>
				<Button
					variant="primary"
					disabled={running}
					onclick={() => {
						if (found) void scanAgain(found.label);
					}}
				>
					Scan again
				</Button>
			</div>
		</header>
		{#if scan.error}
			<Alert tone="error" title="The scan failed">{scan.error}</Alert>
		{/if}
		{#if !found.latest && scan.status !== 'Running'}
			<p class="note">A newer scan of {found.label} exists.</p>
		{/if}
		<div class="tiles">
			<StatTile label="Files" value={count.format(scan.files)} />
			<StatTile label="Size" value={formatBytes(scan.bytes)} />
			<StatTile label="Took" value={took ?? '—'} />
			<StatTile label="Warnings" value={count.format(scan.warnings)} />
		</div>
		<section class="warnings" aria-labelledby="scan-warnings">
			<h3 id="scan-warnings">Warnings</h3>
			{#if scan.warnings === 0}
				<p class="meta">This scan recorded no warnings.</p>
			{:else}
				<div class="pane">
					<DataTable
						source={scanWarnings(api, scan.scanId)}
						columns={warningColumns}
						label="Warnings of this scan"
						empty="No warnings."
						fit
					/>
				</div>
			{/if}
		</section>
	</div>
{/if}

<style>
	.sy-scan-detail {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-16);
	}

	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--sy-space-16);
	}

	.title {
		display: grid;
		gap: var(--sy-space-8);
		min-width: 0;
	}

	.name {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--sy-space-8);
	}

	h2 {
		margin: 0;
		font-size: var(--sy-font-size-heading);
	}

	.meta,
	.note {
		margin: 0;
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
	}

	.actions {
		display: flex;
		align-items: center;
		gap: var(--sy-space-8);
	}

	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(var(--sy-stat-tile-min-width), 1fr));
		gap: var(--sy-space-8);
	}

	.warnings {
		display: grid;
		gap: var(--sy-space-8);
	}

	h3 {
		margin: 0;
		font-size: var(--sy-font-size-body);
		font-weight: var(--sy-font-weight-semibold);
	}

	.pane {
		height: var(--sy-pane-height);
	}
</style>
