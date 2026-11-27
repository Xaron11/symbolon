<!-- Spec: docs/design/screens/add-location-dialog.md -->
<script lang="ts">
	import { untrack } from 'svelte';
	import {
		backend,
		toApiError,
		type DeviceView,
		type HashPolicy,
		type NewLocationRequest
	} from '../api/index.ts';
	import Alert from '../components/Alert/Alert.svelte';
	import Button from '../components/Button/Button.svelte';
	import Dialog from '../components/Dialog/Dialog.svelte';
	import Input from '../components/Input/Input.svelte';
	import Select from '../components/Select/Select.svelte';
	import Tabs from '../components/Tabs/Tabs.svelte';
	import { explain } from './status';

	interface Props {
		open?: boolean;
		/** Called after a location was added. */
		onadded: () => void;
	}

	let { open = $bindable(false), onadded }: Props = $props();
	const api = backend();
	const formId = $props.id();

	let kind = $state<'folder' | 'android'>('folder');
	let path = $state('');
	let serial = $state<string | undefined>(undefined);
	let phonePath = $state('');
	let label = $state('');
	let policy = $state<HashPolicy>('Lazy');
	let algo = $state('default');
	let patterns = $state('');
	let devices = $state<DeviceView[] | undefined>(undefined);
	let devicesError = $state<string | undefined>(undefined);
	let errors = $state<{ path?: string; device?: string; phonePath?: string; label?: string }>({});
	let formError = $state<string | undefined>(undefined);
	let busy = $state(false);

	// The label follows the folder's name until someone edits it.
	let suggested = '';
	const lastPart = (text: string) =>
		text
			.replace(/[/\\]+$/, '')
			.split(/[/\\]/)
			.at(-1) ?? '';
	$effect(() => {
		const next = lastPart(kind === 'folder' ? path : phonePath);
		untrack(() => {
			if (label === suggested) label = next;
			suggested = next;
		});
	});

	function loadDevices() {
		devices = undefined;
		devicesError = undefined;
		api.devices.list().then(
			(list) => (devices = list),
			(e: unknown) => (devicesError = explain(toApiError(e)))
		);
	}

	// Phones come and go: read them each time the dialog opens on the Android tab.
	$effect(() => {
		if (open && kind === 'android') untrack(loadDevices);
	});

	async function choose() {
		const picked = await api.system.pickFolder();
		if (picked !== undefined) path = picked;
	}

	function request(): NewLocationRequest | undefined {
		const found: typeof errors = {};
		if (kind === 'folder' && path.trim() === '') found.path = 'Choose a folder.';
		if (kind === 'android' && serial === undefined) found.device = 'Choose a phone.';
		if (kind === 'android' && phonePath.trim() === '')
			found.phonePath = 'Enter a path on the phone.';
		if (label.trim() === '') found.label = 'Enter a label.';
		errors = found;
		if (Object.keys(found).length > 0) return undefined;
		return {
			label: label.trim(),
			kind:
				kind === 'folder'
					? { type: 'Folder', path: path.trim() }
					: { type: 'Android', serial: serial ?? '', path: phonePath.trim() },
			hashPolicy: policy,
			hashAlgo: algo === 'default' ? null : algo,
			ignorePatterns: patterns
				.split(',')
				.map((p) => p.trim())
				.filter((p) => p !== '')
		};
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		formError = undefined;
		const next = request();
		if (!next) return;
		busy = true;
		try {
			await api.locations.add(next);
			open = false;
			path = phonePath = label = patterns = suggested = '';
			serial = undefined;
			onadded();
		} catch (e) {
			// What was typed stays; only the field at fault is marked.
			const error = toApiError(e);
			if (error.kind === 'AlreadyExists') errors = { label: explain(error) };
			else if (error.kind === 'InvalidInput' && /root|path/i.test(error.message)) {
				errors = kind === 'folder' ? { path: error.message } : { phonePath: error.message };
			} else formError = explain(error);
		} finally {
			busy = false;
		}
	}
</script>

{#snippet folderFields()}
	<div class="row">
		<Input label="Folder path" bind:value={path} {...errors.path ? { error: errors.path } : {}} />
		<Button
			variant="secondary"
			onclick={() => {
				void choose();
			}}
		>
			Choose…
		</Button>
	</div>
{/snippet}

{#snippet androidFields()}
	<fieldset class="devices">
		<legend>Phone</legend>
		{#if devicesError}
			<p class="error" role="alert">{devicesError}</p>
		{:else if devices === undefined}
			<p class="muted">Looking for phones…</p>
		{:else if devices.length === 0}
			<p class="muted">No phones found. Connect one over USB with USB debugging on.</p>
		{:else}
			{#each devices as device (device.serial)}
				<label class="device">
					<input
						type="radio"
						name="{formId}-device"
						value={device.serial}
						disabled={!device.ready}
						bind:group={serial}
					/>
					<span>
						{device.model ?? device.serial} <span class="sy-mono muted">{device.serial}</span>
					</span>
				</label>
				{#if device.hint}<p class="muted hint">{device.hint}</p>{/if}
			{/each}
		{/if}
		{#if errors.device}<p class="error">{errors.device}</p>{/if}
		<div>
			<Button size="sm" variant="ghost" onclick={loadDevices}>Look again</Button>
		</div>
	</fieldset>
	<Input
		label="Path on the phone"
		bind:value={phonePath}
		{...errors.phonePath ? { error: errors.phonePath } : {}}
	/>
{/snippet}

<Dialog bind:open title="Add location">
	<form id={formId} class="form" onsubmit={submit} novalidate>
		<Tabs
			label="Kind of location"
			bind:value={kind}
			tabs={[
				{ value: 'folder', label: 'Folder', content: folderFields },
				{ value: 'android', label: 'Android device', content: androidFields }
			]}
		/>
		<Input
			label="Label"
			hint="Defaults to the folder's name."
			bind:value={label}
			{...errors.label ? { error: errors.label } : {}}
		/>
		<Select
			label="When to hash files"
			bind:value={policy}
			options={[
				{ value: 'Lazy', label: 'When a comparison needs it' },
				{ value: 'Eager', label: 'During every scan' }
			]}
		/>
		<details class="advanced">
			<summary>Advanced</summary>
			<div class="advanced-body">
				<Select
					label="Hash algorithm"
					bind:value={algo}
					options={[
						{ value: 'default', label: 'BLAKE3 (default)' },
						{ value: 'xxh3', label: 'XXH3' },
						{ value: 'sha256', label: 'SHA-256' }
					]}
				/>
				<Input
					label="Ignore patterns"
					hint="Comma-separated, in .gitignore syntax, such as *.tmp, node_modules/"
					bind:value={patterns}
				/>
			</div>
		</details>
		{#if formError}<Alert tone="error" title="Could not add the location">{formError}</Alert>{/if}
	</form>
	{#snippet footer()}
		<Button
			variant="ghost"
			onclick={() => {
				open = false;
			}}
		>
			Cancel
		</Button>
		<Button variant="primary" type="submit" form={formId} loading={busy}>Add</Button>
	{/snippet}
</Dialog>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-12);
	}

	.advanced summary {
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		cursor: pointer;
	}

	.advanced-body {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-12);
		margin-top: var(--sy-space-12);
	}

	.row {
		display: flex;
		align-items: flex-end;
		gap: var(--sy-space-8);
	}

	.row > :global(:first-child) {
		flex: 1;
	}

	.devices {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-4);
		margin: 0 0 var(--sy-space-12);
		padding: 0;
		border: 0;
	}

	legend {
		margin-bottom: var(--sy-space-4);
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
	}

	.device {
		display: flex;
		align-items: center;
		gap: var(--sy-space-8);
	}

	.hint {
		margin: 0 0 var(--sy-space-4) var(--sy-space-24);
	}

	.muted,
	.error {
		margin: 0;
		font-size: var(--sy-font-size-small);
	}

	.muted {
		color: var(--sy-color-text-muted);
	}

	.error {
		color: var(--sy-status-error-fg);
	}
</style>
