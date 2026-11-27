<!-- Spec: docs/design/components/location-detail.md -->
<script lang="ts">
	import { File, Folder } from '@lucide/svelte';
	import {
		backend,
		catalogTree,
		locationScans,
		scanWarnings,
		toApiError,
		type EntryView,
		type LocationView,
		type ScanBrief,
		type WarningView
	} from '../api/index.ts';
	import Alert from '../components/Alert/Alert.svelte';
	import Badge from '../components/Badge/Badge.svelte';
	import Button from '../components/Button/Button.svelte';
	import DataTable, { type Column } from '../components/DataTable/DataTable.svelte';
	import EmptyState from '../components/EmptyState/EmptyState.svelte';
	import Spinner from '../components/Spinner/Spinner.svelte';
	import StatTile from '../components/StatTile/StatTile.svelte';
	import Tabs from '../components/Tabs/Tabs.svelte';
	import TreeView from '../components/TreeView/TreeView.svelte';
	import { toasts } from '../feedback/toasts.svelte';
	import { formatAgo, formatBytes } from '../format';
	import { scans, statusText } from '../scans/scans.svelte';
	import { explain, scanBlocked, statusInfo } from './status';

	interface Props {
		label: string;
		/** Opens a route, e.g. Scans when a scan already runs. */
		navigate: (href: string) => void;
	}

	let { label, navigate }: Props = $props();
	const api = backend();
	const scanState = scans();
	const count = new Intl.NumberFormat('en-US');

	/** Undefined while loading, null once the location is known to be gone. */
	let location = $state<LocationView | null | undefined>(undefined);
	let loadError = $state<string | undefined>(undefined);
	let tab = $state<'browse' | 'scans' | 'warnings'>('browse');

	async function load() {
		try {
			location = (await api.locations.list()).find((l) => l.label === label) ?? null;
			loadError = undefined;
		} catch (e) {
			loadError = explain(toApiError(e));
		}
	}

	// Bumped when a scan of this location ends, so the history table is read again.
	let historyGeneration = $state(0);

	void load();
	$effect(() =>
		scanState.onFinished((update) => {
			if (update.label !== label) return;
			historyGeneration++;
			void load();
		})
	);

	const running = $derived(scanState.running.filter((u) => u.label === label));
	// svelte-ignore state_referenced_locally
	const tree = catalogTree(api, label);
	// svelte-ignore state_referenced_locally
	const history = locationScans(api, label);

	const hashed = (l: LocationView) =>
		l.stats.files === 0
			? '0%'
			: `${String(Math.floor((l.stats.verifiedFiles / l.stats.files) * 100))}%`;

	async function scan() {
		try {
			await scanState.start(label);
		} catch (e) {
			const error = toApiError(e);
			if (error.kind === 'AlreadyScanning') navigate('/scans');
			else toasts.show(explain(error), 'error');
		}
	}

	const scanColumns: Column<ScanBrief>[] = [
		{ id: 'started', header: 'Started', cell: (s: ScanBrief) => formatAgo(s.startedMs) },
		{ id: 'status', header: 'Outcome', cell: (s: ScanBrief) => s.status, width: 120 },
		{
			id: 'files',
			header: 'Files',
			cell: (s: ScanBrief) => count.format(s.files),
			mono: true,
			align: 'end',
			width: 110
		},
		{
			id: 'bytes',
			header: 'Size',
			cell: (s: ScanBrief) => formatBytes(s.bytes),
			mono: true,
			align: 'end',
			width: 110
		},
		{
			id: 'warnings',
			header: 'Warnings',
			cell: (s: ScanBrief) => count.format(s.warnings),
			mono: true,
			align: 'end',
			width: 110
		}
	];
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

{#snippet browse()}
	<div class="pane">
		<TreeView
			source={tree}
			name="Files of {label}"
			label={(entry: EntryView) => entry.name}
			aside={(entry: EntryView) => (entry.folder ? undefined : formatBytes(entry.size))}
			empty="Nothing scanned here yet."
			icon={(entry: EntryView) => (entry.folder ? Folder : File)}
		/>
	</div>
{/snippet}

{#snippet scanList()}
	<div class="pane">
		{#key historyGeneration}
			<DataTable
				source={history}
				columns={scanColumns}
				label="Scans of {label}"
				empty="Not scanned yet."
				fit
			/>
		{/key}
	</div>
{/snippet}

{#snippet warningList()}
	<div class="pane">
		{#if location?.lastScan}
			{#key location.lastScan.scanId}
				<DataTable
					source={scanWarnings(api, location.lastScan.scanId)}
					columns={warningColumns}
					label="Warnings of the last scan"
					empty="No warnings in the last scan."
					fit
				/>
			{/key}
		{:else}
			<p class="muted">No scans yet.</p>
		{/if}
	</div>
{/snippet}

<nav class="trail" aria-label="Breadcrumb">
	<a href="/locations">Locations</a>
	<span aria-hidden="true">›</span>
	<span aria-current="page">{label}</span>
</nav>

{#if loadError}
	<Alert tone="error" title="Could not read the location">{loadError}</Alert>
{:else if location === undefined}
	<Spinner label="Loading {label}" />
{:else if location === null}
	<EmptyState title="This location no longer exists.">
		{#snippet actions()}<a href="/locations">Back to Locations</a>{/snippet}
	</EmptyState>
{:else}
	{@const status = statusInfo(location.status)}
	{@const blocked = scanBlocked(location.status)}
	{@const Icon = status.icon}
	<div class="detail">
		<header class="head">
			<div>
				<h2>{location.label}</h2>
				<p class="sy-mono muted root">{location.root}</p>
			</div>
			<Badge tone={status.tone}><Icon aria-hidden="true" />{status.label}</Badge>
			<div class="spacer"></div>
			{#if running.length > 0}<p class="muted">{statusText(running)}</p>{/if}
			<Button
				variant="primary"
				disabled={running.length > 0 || blocked !== undefined}
				title={blocked}
				onclick={() => {
					void scan();
				}}
			>
				Scan
			</Button>
		</header>
		<div class="tiles">
			<StatTile label="Files" value={count.format(location.stats.files)} />
			<StatTile label="Folders" value={count.format(location.stats.dirs)} />
			<StatTile label="Size" value={formatBytes(location.stats.bytes)} />
			<StatTile label="Hashed" value={hashed(location)} />
		</div>
		<Tabs
			label="Location views"
			bind:value={tab}
			tabs={[
				{ value: 'browse', label: 'Browse', content: browse },
				{ value: 'scans', label: 'Scans', content: scanList },
				{ value: 'warnings', label: 'Warnings', content: warningList }
			]}
		/>
	</div>
{/if}

<style>
	.trail {
		display: flex;
		gap: var(--sy-space-8);
		margin-bottom: var(--sy-space-12);
		color: var(--sy-color-text-muted);
		font-size: var(--sy-font-size-small);
	}

	.trail a {
		color: var(--sy-color-text-secondary);
		text-decoration: none;
	}

	.trail a:hover {
		color: var(--sy-color-text-primary);
		text-decoration: underline;
	}

	.detail {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-16);
		height: 100%;
		min-height: 0;
	}

	.head {
		display: flex;
		align-items: center;
		gap: var(--sy-space-12);
	}

	h2 {
		margin: 0;
		font-size: var(--sy-font-size-heading);
	}

	.root {
		margin: 0;
	}

	.spacer {
		flex: 1;
	}

	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(var(--sy-table-column-min), 1fr));
		gap: var(--sy-space-12);
	}

	.pane {
		height: var(--sy-pane-height);
		min-height: 0;
	}

	.muted {
		margin: 0;
		color: var(--sy-color-text-muted);
	}
</style>
