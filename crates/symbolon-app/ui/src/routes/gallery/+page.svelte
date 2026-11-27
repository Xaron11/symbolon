<script lang="ts">
	import { Settings, Trash, FolderOpen } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import Alert from '#lib/components/Alert/Alert.svelte';
	import Badge from '#lib/components/Badge/Badge.svelte';
	import Button from '#lib/components/Button/Button.svelte';
	import CategoryChip from '#lib/components/CategoryChip/CategoryChip.svelte';
	import ChartFrame from '#lib/components/ChartFrame/ChartFrame.svelte';
	import Checkbox from '#lib/components/Checkbox/Checkbox.svelte';
	import Combobox from '#lib/components/Combobox/Combobox.svelte';
	import CompositionBar from '#lib/components/CompositionBar/CompositionBar.svelte';
	import DataTable from '#lib/components/DataTable/DataTable.svelte';
	import Dialog from '#lib/components/Dialog/Dialog.svelte';
	import DropdownMenu from '#lib/components/DropdownMenu/DropdownMenu.svelte';
	import EmptyState from '#lib/components/EmptyState/EmptyState.svelte';
	import IconButton from '#lib/components/IconButton/IconButton.svelte';
	import Input from '#lib/components/Input/Input.svelte';
	import Kbd from '#lib/components/Kbd/Kbd.svelte';
	import Popover from '#lib/components/Popover/Popover.svelte';
	import ProgressBar from '#lib/components/ProgressBar/ProgressBar.svelte';
	import Select from '#lib/components/Select/Select.svelte';
	import Spinner from '#lib/components/Spinner/Spinner.svelte';
	import StatTile from '#lib/components/StatTile/StatTile.svelte';
	import Switch from '#lib/components/Switch/Switch.svelte';
	import Tabs from '#lib/components/Tabs/Tabs.svelte';
	import Tooltip from '#lib/components/Tooltip/Tooltip.svelte';
	import TreeView from '#lib/components/TreeView/TreeView.svelte';
	import { CATEGORIES } from '#lib/compare/categories.ts';
	import { memorySource, memoryTree, type MemoryNode } from '#lib/data/memory.ts';
	import { toasts } from '#lib/feedback/toasts.svelte.ts';
	import { formatBytes } from '#lib/format.ts';

	interface File {
		path: string;
		size: number;
		hash: string;
	}

	const files: File[] = Array.from({ length: 100_000 }, (_, i) => ({
		path:
			i === 3
				? `photos/${'very-long-name-'.repeat(20)}.jpg`
				: `photos/${String(2000 + (i % 25))}/IMG_${String(i).padStart(6, '0')}.jpg`,
		size: (i * 7919) % 50_000_000,
		hash: (i * 2654435761).toString(16).padStart(16, '0')
	}));
	const columns = [
		{ id: 'path', header: 'Path', cell: (f: File) => f.path, sortable: true },
		{
			id: 'size',
			header: 'Size',
			cell: (f: File) => formatBytes(f.size),
			mono: true,
			align: 'end' as const,
			width: 100
		},
		{ id: 'hash', header: 'BLAKE3', cell: (f: File) => f.hash, mono: true, width: 180 }
	];
	const tree: MemoryNode[] = [
		{
			key: 'photos',
			label: 'photos',
			children: Array.from({ length: 25 }, (_, y) => ({
				key: `photos/${String(2000 + y)}`,
				label: String(2000 + y),
				children: Array.from({ length: 2000 }, (_, i) => ({
					key: `photos/${String(2000 + y)}/${String(i)}`,
					label: `IMG_${String(i).padStart(6, '0')}.jpg`
				}))
			}))
		},
		{ key: 'notes.txt', label: 'notes.txt' }
	];
	const segments = [
		{ category: 'identical' as const, count: 182_311, bytes: 412_000_000_000 },
		{ category: 'different' as const, count: 1_204, bytes: 3_200_000_000 },
		{ category: 'only_in' as const, count: 9_812, bytes: 22_000_000_000 },
		{ category: 'moved' as const, count: 330 },
		{ category: 'renamed' as const, count: 41 },
		{ category: 'collision' as const, count: 1 },
		{ category: 'unverified' as const, count: 77 }
	];

	let dialogOpen = $state(false);
	let checked = $state(true);
	let toggled = $state(false);
	let algo = $state<'blake3' | 'xxh3' | 'sha256'>('blake3');
	let location = $state<string | undefined>('laptop');
	let tab = $state<'summary' | 'files'>('summary');
	let name = $state('photos');
</script>

<svelte:head><title>Gallery · Symbolon</title></svelte:head>

{#snippet both(title: string, content: Snippet)}
	<section>
		<h2>{title}</h2>
		<div class="themes">
			<div class="pane" data-theme="light">{@render content()}</div>
			<div class="pane" data-theme="dark">{@render content()}</div>
		</div>
	</section>
{/snippet}

<div class="gallery">
	<p>
		Every component in both themes. Overlays open in the page's theme (they render outside these
		panes).
	</p>

	{#snippet buttons()}
		<div class="row">
			<Button variant="primary">Compare</Button>
			<Button>Rescan</Button>
			<Button variant="ghost">Cancel</Button>
			<Button variant="danger">Remove</Button>
			<Button size="sm">Small</Button>
			<Button loading>Saving</Button>
			<Button disabled>Disabled</Button>
			<IconButton label="Settings" icon={Settings} />
			<IconButton label="Remove" icon={Trash} variant="danger" />
			<IconButton label="Open" icon={FolderOpen} variant="secondary" size="sm" />
		</div>
	{/snippet}
	{@render both('Buttons', buttons)}

	{#snippet fields()}
		<div class="grid">
			<Input label="Label" bind:value={name} />
			<Input label="Filter" type="search" placeholder="Path contains…" />
			<Input label="Root" value="/mnt/usb" error="Not a folder" />
			<Select
				label="Hash algorithm"
				bind:value={algo}
				options={[
					{ value: 'blake3', label: 'BLAKE3' },
					{ value: 'xxh3', label: 'XXH3' },
					{ value: 'sha256', label: 'SHA-256' }
				]}
			/>
			<Combobox
				label="Location"
				bind:value={location}
				options={['laptop', 'backup', 'phone'].map((v) => ({ value: v, label: v }))}
			/>
			<div class="row">
				<Checkbox label="Include hidden files" bind:checked />
				<Switch label="Hash eagerly" bind:checked={toggled} />
			</div>
		</div>
	{/snippet}
	{@render both('Fields', fields)}

	{#snippet display()}
		<div class="grid">
			<div class="row">
				{#each CATEGORIES as c (c.name)}<CategoryChip category={c.name} count={1234} />{/each}
			</div>
			<div class="row">
				{#each CATEGORIES as c (c.name)}<CategoryChip category={c.name} compact />{/each}
				<Badge>12</Badge><Badge tone="accent">3</Badge><Badge tone="error">1</Badge><Badge
					tone="warning"
				>
					4
				</Badge><Badge tone="success">ok</Badge><Badge tone="info">new</Badge>
				<Kbd keys="Ctrl+K" /><Kbd keys="g l" /><Spinner />
			</div>
			<ProgressBar label="Scanning laptop" value={0.42} />
			<ProgressBar label="Listing phone" />
			<Alert tone="error" title="Scan failed">The drive was removed.</Alert>
			<Alert tone="warning" title="3 files could not be read" />
			<Alert tone="success" title="Backup is complete" />
			<div class="row">
				<StatTile label="Identical" value="182,311" detail="412 GB" category="identical" />
				<StatTile label="Different" value="1,204" detail="3.2 GB" category="different" />
				<StatTile label="Entries" value="193,776" />
			</div>
			<EmptyState
				title="No locations yet"
				description="Add a folder, a drive or a phone to compare."
			/>
		</div>
	{/snippet}
	{@render both('Display', display)}

	{#snippet summary()}Summary panel{/snippet}
	{#snippet filesPanel()}Files panel{/snippet}
	{#snippet navigation()}
		<div class="grid">
			<Tabs
				label="Comparison"
				bind:value={tab}
				tabs={[
					{ value: 'summary', label: 'Summary', content: summary },
					{ value: 'files', label: 'Files', content: filesPanel }
				]}
			/>
			<CompositionBar label="Comparison of laptop and backup" {segments} showTable />
		</div>
	{/snippet}
	{@render both('Navigation and summary', navigation)}

	{#snippet overlays()}
		{#snippet actionsTrigger()}Actions{/snippet}
		{#snippet detailsTrigger()}Details{/snippet}
		<div class="row">
			<Button onclick={() => (dialogOpen = true)}>Open dialog</Button>
			<DropdownMenu
				label="Location actions"
				trigger={actionsTrigger}
				items={[
					{ label: 'Rescan', onSelect: () => toasts.show('Rescan started'), shortcut: 'r' },
					{ label: 'Export', onSelect: () => toasts.show('Exported', 'success') },
					{ label: 'Disabled', onSelect: () => undefined, disabled: true },
					{ separator: true },
					{
						label: 'Remove',
						onSelect: () => toasts.show('Could not remove', 'error'),
						danger: true
					}
				]}
			/>
			<Popover label="Scan details" trigger={detailsTrigger}>
				Scanned 3 minutes ago, 193,776 entries.
			</Popover>
			<Tooltip text="Rescan all locations">
				{#snippet children(props)}<Button {...props}>Hover or focus me</Button>{/snippet}
			</Tooltip>
		</div>
	{/snippet}
	{@render both('Overlays', overlays)}

	{#snippet data()}
		<div class="data">
			<DataTable
				source={memorySource(files)}
				{columns}
				label="Files ({files.length.toLocaleString('en-US')})"
			/>
			<TreeView source={memoryTree(tree)} label={(n: MemoryNode) => n.label} name="Folders" />
		</div>
	{/snippet}
	{@render both('Data (100,000 rows)', data)}

	{#snippet chart()}
		{#snippet chartTable()}<p>jpg 812, png 210, mov 33</p>{/snippet}
		<ChartFrame
			label="Files by extension"
			table={chartTable}
			config={(p: Readonly<Record<string, string>>) => ({
				type: 'bar',
				data: {
					labels: ['jpg', 'png', 'mov'],
					datasets: [
						{ label: 'Files', data: [812, 210, 33], backgroundColor: p['color.accent.default'] }
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
	{/snippet}
	{@render both('Chart', chart)}
</div>

<Dialog bind:open={dialogOpen} title="Add location" description="A folder, a drive or a phone.">
	<Input label="Path" />
	{#snippet footer()}
		<Button onclick={() => (dialogOpen = false)}>Cancel</Button>
		<Button variant="primary" onclick={() => (dialogOpen = false)}>Add</Button>
	{/snippet}
</Dialog>

<style>
	.gallery {
		display: grid;
		gap: var(--sy-space-24);
	}

	h2 {
		margin: 0 0 var(--sy-space-8);
		font-size: var(--sy-font-size-title);
	}

	.themes {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--sy-space-12);
	}

	.pane {
		min-width: 0;
		padding: var(--sy-space-16);
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-canvas);
		color: var(--sy-color-text-primary);
	}

	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--sy-space-8);
	}

	.grid {
		display: grid;
		gap: var(--sy-space-12);
	}

	.data {
		display: grid;
		grid-template-rows: calc(var(--sy-table-row-height) * 12) calc(var(--sy-table-row-height) * 10);
		gap: var(--sy-space-12);
	}
</style>
