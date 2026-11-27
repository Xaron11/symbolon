<!-- Spec: docs/design/screens/location-detail.md -->
<script lang="ts">
	import { Ellipsis, File, Folder, Link } from '@lucide/svelte';
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
	import Breadcrumb from '../components/Breadcrumb/Breadcrumb.svelte';
	import Button from '../components/Button/Button.svelte';
	import DropdownMenu from '../components/DropdownMenu/DropdownMenu.svelte';
	import DataTable, { type Column } from '../components/DataTable/DataTable.svelte';
	import EmptyState from '../components/EmptyState/EmptyState.svelte';
	import Spinner from '../components/Spinner/Spinner.svelte';
	import StatTile from '../components/StatTile/StatTile.svelte';
	import Tabs from '../components/Tabs/Tabs.svelte';
	import TreeView from '../components/TreeView/TreeView.svelte';
	import { toasts } from '../feedback/toasts.svelte';
	import { formatAgo, formatBytes } from '../format';
	import { outcomeColor, outcomeLabel, scanDuration } from '../scans/progress';
	import { scans, statusText } from '../scans/scans.svelte';
	import RemoveDialog from './RemoveDialog.svelte';
	import RenameDialog from './RenameDialog.svelte';
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
	let renaming = $state<string | undefined>(undefined);
	let removing = $state<string | undefined>(undefined);

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

	async function setPolicy(eager: boolean) {
		try {
			await api.locations.setHashPolicy(label, eager ? 'Eager' : 'Lazy');
			await load();
		} catch (e) {
			toasts.show(explain(toApiError(e)), 'error');
		}
	}

	// The latest ended scan: a running one is not it, its counts are not final.
	const latestId = $derived(
		location?.lastScan && location.lastScan.status !== 'Running'
			? location.lastScan.scanId
			: undefined
	);

	async function scan() {
		try {
			await scanState.start(label);
		} catch (e) {
			const error = toApiError(e);
			if (error.kind === 'AlreadyScanning') navigate('/scans');
			else toasts.show(explain(error), 'error');
		}
	}

	const scanColumns = $derived<Column<ScanBrief>[]>([
		{
			id: 'started',
			header: 'Started',
			cell: (s: ScanBrief) => formatAgo(s.startedMs),
			badge: (s: ScanBrief) => (s.scanId === latestId ? 'Latest' : undefined)
		},
		{
			id: 'took',
			header: 'Took',
			cell: (s: ScanBrief) => scanDuration(s) ?? '—',
			width: 100
		},
		{
			id: 'status',
			header: 'Outcome',
			cell: (s: ScanBrief) => outcomeLabel(s.status),
			marker: (s: ScanBrief) => outcomeColor(s.status),
			width: 120
		},
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
	]);
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
			icon={(entry: EntryView) => (entry.folder ? Folder : entry.kind === 'Symlink' ? Link : File)}
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
				openOnClick
				onopen={(s: ScanBrief) => {
					navigate(`/scans/${String(s.scanId)}`);
				}}
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

<div class="trail">
	<Breadcrumb trail={[{ label: 'Locations', href: '/locations' }]} current={label} />
</div>

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
	{@const last = location.lastScan}
	<div class="detail">
		<header class="head">
			<div class="title">
				<div class="name">
					<h2>{location.label}</h2>
					<Badge tone={status.tone}><Icon aria-hidden="true" />{status.label}</Badge>
				</div>
				<p class="sy-mono root" title={location.root}>{location.root}</p>
				<p class="meta">
					{#if running.length > 0}
						<span>{statusText(running)}</span>
					{:else if last}
						<span>
							Last scanned <a href="/scans/{last.scanId}">
								{formatAgo(last.finishedMs ?? last.startedMs)}
							</a>
						</span>
						<span>{outcomeLabel(last.status)}</span>
					{:else}
						<span>Never scanned</span>
					{/if}
					<span>
						Hashed {location.hashPolicy === 'Eager' ? 'during every scan' : 'when compared'}
					</span>
				</p>
			</div>
			<div class="actions">
				<Button
					variant="primary"
					loading={running.length > 0}
					disabled={blocked !== undefined}
					title={blocked}
					onclick={() => {
						void scan();
					}}
				>
					Scan
				</Button>
				<DropdownMenu
					label="Actions for {location.label}"
					items={[
						{ label: 'Rename…', onSelect: () => (renaming = location?.label) },
						{
							label:
								location.hashPolicy === 'Eager'
									? 'Hash only when compared'
									: 'Hash during every scan',
							onSelect: () => void setPolicy(location?.hashPolicy !== 'Eager')
						},
						{ separator: true },
						{ label: 'Remove…', danger: true, onSelect: () => (removing = location?.label) }
					]}
				>
					{#snippet trigger()}<Ellipsis aria-hidden="true" />{/snippet}
				</DropdownMenu>
			</div>
		</header>
		{#if blocked}<p class="blocked">{blocked}</p>{/if}
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
				{
					value: 'warnings',
					label: last && last.warnings > 0 ? `Warnings ${count.format(last.warnings)}` : 'Warnings',
					content: warningList
				}
			]}
		/>
	</div>
{/if}

<RenameDialog
	bind:label={renaming}
	onrenamed={(to: string) => {
		navigate(`/locations/${encodeURIComponent(to)}`);
	}}
/>
<RemoveDialog
	bind:label={removing}
	onremoved={() => {
		navigate('/locations');
	}}
/>

<style>
	.trail {
		margin-bottom: var(--sy-space-12);
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
		flex-wrap: wrap;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--sy-space-16);
	}

	.title {
		display: grid;
		gap: var(--sy-space-8);
		min-width: 0;
		flex: 1;
	}

	.name {
		display: flex;
		align-items: center;
		gap: var(--sy-space-12);
	}

	h2 {
		margin: 0;
		font-size: var(--sy-font-size-heading);
	}

	/* One line; the full path is in the tooltip. */
	.root {
		margin: 0;
		overflow: hidden;
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta,
	.blocked {
		margin: 0;
		color: var(--sy-color-text-muted);
		font-size: var(--sy-font-size-small);
	}

	.meta span + span::before {
		content: ' · ';
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

	.pane {
		height: var(--sy-pane-height);
		min-height: 0;
	}

	.muted {
		margin: 0;
		color: var(--sy-color-text-muted);
	}
</style>
