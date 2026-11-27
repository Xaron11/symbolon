<!-- Spec: docs/design/components/location-table.md -->
<script lang="ts">
	import { Ellipsis } from '@lucide/svelte';
	import type { HashPolicy, LocationView } from '../api/index.ts';
	import Badge from '../components/Badge/Badge.svelte';
	import Button from '../components/Button/Button.svelte';
	import DropdownMenu from '../components/DropdownMenu/DropdownMenu.svelte';
	import type { MenuItem } from '../components/menu';
	import { formatAgo, formatBytes } from '../format';
	import { outcomeLabel } from '../scans/progress';
	import { scanBlocked, statusInfo } from './status';

	interface Props {
		locations: readonly LocationView[];
		/** Labels of locations being scanned now. */
		scanning?: readonly string[];
		onscan: (label: string) => void;
		onrename: (label: string) => void;
		onpolicy: (label: string, policy: HashPolicy) => void;
		onremove: (label: string) => void;
		/** Points a moved location at its new place, or at candidate `choice`. */
		onfollow: (label: string, choice?: number) => void;
		/** Opens a location's page; the whole row is a target, the label link is for keyboards. */
		onopen: (label: string) => void;
	}

	let {
		locations,
		scanning = [],
		onscan,
		onrename,
		onpolicy,
		onremove,
		onfollow,
		onopen
	}: Props = $props();
	const count = new Intl.NumberFormat('en-US');

	function items(location: LocationView): MenuItem[] {
		const label = location.label;
		const eager = location.hashPolicy === 'Eager';
		const candidates =
			location.status.type === 'Ambiguous'
				? location.status.candidates.map((place, i) => ({
						label: `Use ${place}`,
						onSelect: () => {
							onfollow(label, i);
						}
					}))
				: [];
		return [
			...(candidates.length > 0 ? [...candidates, { separator: true } as const] : []),
			{
				label: 'Rename…',
				onSelect: () => {
					onrename(label);
				}
			},
			{
				label: eager ? 'Hash only when compared' : 'Hash during every scan',
				onSelect: () => {
					onpolicy(label, eager ? 'Lazy' : 'Eager');
				}
			},
			{ separator: true },
			{
				label: 'Remove…',
				danger: true,
				onSelect: () => {
					onremove(label);
				}
			}
		];
	}
</script>

<table class="sy-locations">
	<thead>
		<tr>
			<th scope="col">Location</th>
			<th scope="col">Root</th>
			<th scope="col">Status</th>
			<th scope="col">Last scan</th>
			<th scope="col" class="end">Files</th>
			<th scope="col" class="end">Size</th>
			<th scope="col">Hashing</th>
			<th scope="col"><span class="sy-visually-hidden">Actions</span></th>
		</tr>
	</thead>
	<tbody>
		{#each locations as location (location.label)}
			{@const status = statusInfo(location.status)}
			{@const blocked = scanBlocked(location.status)}
			{@const Icon = status.icon}
			{@const last = location.lastScan}
			<!-- The label link gives keyboards the same target; this only widens it for pointers. -->
			<tr
				onclick={(event) => {
					const target = event.target as Element;
					if (!target.closest('a, button, [role="menuitem"]')) onopen(location.label);
				}}
			>
				<th scope="row">
					<a href="/locations/{encodeURIComponent(location.label)}">{location.label}</a>
				</th>
				<td class="sy-mono root" title={location.root}>{location.root}</td>
				<td>
					<div class="status">
						<Badge tone={status.tone}><Icon aria-hidden="true" />{status.label}</Badge>
						{#if status.action === 'followMove'}
							<Button
								size="sm"
								variant="secondary"
								aria-label="Use new path for {location.label}"
								onclick={() => {
									onfollow(location.label);
								}}
							>
								Use new path
							</Button>
						{/if}
					</div>
					{#if blocked}<p class="note">{blocked}</p>{/if}
				</td>
				<td>
					{#if scanning.includes(location.label)}
						<a class="scan-link" href="/scans" aria-label="Scanning {location.label}">Scanning…</a>
					{:else if last}
						<a
							class="scan-link"
							href="/scans/{last.scanId}"
							title={new Date(last.finishedMs ?? last.startedMs).toLocaleString()}
						>
							{formatAgo(last.finishedMs ?? last.startedMs)}
						</a>
						{#if last.status !== 'Completed'}<span class="note">
								{outcomeLabel(last.status).toLowerCase()}
							</span>{/if}
					{:else}
						<span class="muted">Never</span>
					{/if}
				</td>
				<td class="end sy-mono">{count.format(location.stats.files)}</td>
				<td class="end sy-mono">{formatBytes(location.stats.bytes)}</td>
				<td>{location.hashPolicy === 'Eager' ? 'Every scan' : 'When compared'}</td>
				<td class="end">
					<div class="row-actions">
						<Button
							size="sm"
							variant="secondary"
							aria-label="Scan {location.label}"
							loading={scanning.includes(location.label)}
							disabled={blocked !== undefined}
							onclick={() => {
								onscan(location.label);
							}}
						>
							Scan
						</Button>
						<DropdownMenu label="Actions for {location.label}" items={items(location)} size="sm">
							{#snippet trigger()}<Ellipsis aria-hidden="true" />{/snippet}
						</DropdownMenu>
					</div>
				</td>
			</tr>
		{/each}
	</tbody>
</table>

<style>
	.sy-locations {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--sy-font-size-body);
	}

	th,
	td {
		padding: var(--sy-space-8);
		border-bottom: var(--sy-border-width-default) solid var(--sy-color-border-default);
		text-align: start;
		vertical-align: middle;
		white-space: nowrap;
	}

	thead th {
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		font-weight: var(--sy-font-weight-medium);
	}

	tbody th {
		font-weight: var(--sy-font-weight-medium);
	}

	tbody tr {
		cursor: pointer;
	}

	tbody tr:hover {
		background: var(--sy-color-surface-hover);
	}

	th a {
		color: var(--sy-color-text-primary);
		text-decoration: none;
	}

	tr:hover th a {
		text-decoration: underline;
	}

	/* Takes what the other columns leave and cuts the path there. */
	.root {
		width: 100%;
		max-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.status {
		display: flex;
		align-items: center;
		gap: var(--sy-space-8);
	}

	.end {
		text-align: end;
	}

	.row-actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--sy-space-4);
	}

	.scan-link {
		color: inherit;
		text-decoration: none;
	}

	.scan-link:hover {
		color: var(--sy-color-accent-default);
		text-decoration: underline;
	}

	.note,
	.muted {
		margin: 0;
		color: var(--sy-color-text-muted);
		font-size: var(--sy-font-size-small);
	}
</style>
