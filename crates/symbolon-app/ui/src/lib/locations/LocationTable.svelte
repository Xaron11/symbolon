<!-- Spec: docs/design/components/location-table.md -->
<script lang="ts">
	import { Ellipsis } from '@lucide/svelte';
	import type { HashPolicy, LocationView } from '../api/index.ts';
	import Badge from '../components/Badge/Badge.svelte';
	import Button from '../components/Button/Button.svelte';
	import DropdownMenu from '../components/DropdownMenu/DropdownMenu.svelte';
	import type { MenuItem } from '../components/menu';
	import { formatAgo, formatBytes } from '../format';
	import { scanBlocked, statusInfo } from './status';

	interface Props {
		locations: readonly LocationView[];
		onscan: (label: string) => void;
		onrename: (label: string) => void;
		onpolicy: (label: string, policy: HashPolicy) => void;
		onremove: (label: string) => void;
		/** Points a moved location at its new place, or at candidate `choice`. */
		onfollow: (label: string, choice?: number) => void;
	}

	let { locations, onscan, onrename, onpolicy, onremove, onfollow }: Props = $props();
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
			{
				label: 'Scan',
				disabled: scanBlocked(location.status) !== undefined,
				onSelect: () => {
					onscan(label);
				}
			},
			...(candidates.length > 0 ? [{ separator: true } as const, ...candidates] : []),
			{ separator: true },
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
			<tr>
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
					{#if last}
						<span title={new Date(last.finishedMs ?? last.startedMs).toLocaleString()}>
							{formatAgo(last.finishedMs ?? last.startedMs)}
						</span>
						{#if last.status !== 'Completed'}<span class="note">
								{last.status.toLowerCase()}
							</span>{/if}
					{:else}
						<span class="muted">Never</span>
					{/if}
				</td>
				<td class="end sy-mono">{count.format(location.stats.files)}</td>
				<td class="end sy-mono">{formatBytes(location.stats.bytes)}</td>
				<td>{location.hashPolicy === 'Eager' ? 'Every scan' : 'When compared'}</td>
				<td class="end">
					<DropdownMenu label="Actions for {location.label}" items={items(location)}>
						{#snippet trigger()}<Ellipsis aria-hidden="true" />{/snippet}
					</DropdownMenu>
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
	}

	thead th {
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		font-weight: var(--sy-font-weight-medium);
	}

	tbody th {
		font-weight: var(--sy-font-weight-medium);
	}

	a {
		color: var(--sy-color-text-primary);
	}

	.root {
		max-width: var(--sy-table-column-min);
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

	.note,
	.muted {
		margin: 0;
		color: var(--sy-color-text-muted);
		font-size: var(--sy-font-size-small);
	}
</style>
