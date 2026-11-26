<!-- Spec: docs/design/components/location-table.md -->
<script lang="ts">
	import { FolderPlus } from '@lucide/svelte';
	import { backend, toApiError, type HashPolicy, type LocationView } from '../api/index.ts';
	import Alert from '../components/Alert/Alert.svelte';
	import Button from '../components/Button/Button.svelte';
	import EmptyState from '../components/EmptyState/EmptyState.svelte';
	import Spinner from '../components/Spinner/Spinner.svelte';
	import { toasts } from '../feedback/toasts.svelte';
	import { scans } from '../scans/scans.svelte';
	import AddLocationDialog from './AddLocationDialog.svelte';
	import LocationTable from './LocationTable.svelte';
	import RemoveDialog from './RemoveDialog.svelte';
	import RenameDialog from './RenameDialog.svelte';
	import { explain } from './status';

	interface Props {
		/** Opens a route, e.g. Scans when a scan already runs. */
		navigate: (href: string) => void;
		/** Opens Add location at once (the palette's "Add location"). */
		openAdd?: boolean;
	}

	let { navigate, openAdd = false }: Props = $props();
	const api = backend();
	const scanState = scans();
	let locations = $state<LocationView[] | undefined>(undefined);
	let loadError = $state<string | undefined>(undefined);
	// svelte-ignore state_referenced_locally
	let adding = $state(openAdd);
	let renaming = $state<string | undefined>(undefined);
	let removing = $state<string | undefined>(undefined);

	async function load() {
		try {
			locations = await api.locations.list();
			loadError = undefined;
		} catch (e) {
			loadError = explain(toApiError(e));
		}
	}

	void load();
	$effect(() =>
		scanState.onFinished(() => {
			void load();
		})
	);

	async function act(work: () => Promise<unknown>) {
		try {
			await work();
			await load();
		} catch (e) {
			const error = toApiError(e);
			if (error.kind === 'AlreadyScanning') navigate('/scans');
			else toasts.show(explain(error), 'error');
		}
	}
</script>

{#if locations !== undefined && locations.length > 0}
	<div class="actions">
		<Button
			variant="primary"
			onclick={() => {
				adding = true;
			}}
		>
			Add location
		</Button>
	</div>
{/if}

{#if loadError}
	<Alert tone="error" title="Could not read the locations">{loadError}</Alert>
{:else if locations === undefined}
	<Spinner label="Loading locations" />
{:else if locations.length === 0}
	<EmptyState
		title="No locations yet"
		description="A location is a folder, drive or phone that Symbolon scans and compares."
		icon={FolderPlus}
	>
		{#snippet actions()}
			<Button
				variant="primary"
				onclick={() => {
					adding = true;
				}}
			>
				Add location
			</Button>
		{/snippet}
	</EmptyState>
{:else}
	<LocationTable
		{locations}
		onscan={(label: string) => void act(() => scanState.start(label))}
		onrename={(label: string) => (renaming = label)}
		onpolicy={(label: string, policy: HashPolicy) =>
			void act(() => api.locations.setHashPolicy(label, policy))}
		onremove={(label: string) => (removing = label)}
		onfollow={(label: string, choice?: number) =>
			void act(() => api.locations.followMove(label, choice))}
	/>
{/if}

<AddLocationDialog bind:open={adding} onadded={() => void load()} />
<RenameDialog bind:label={renaming} onrenamed={() => void load()} />
<RemoveDialog bind:label={removing} onremoved={() => void load()} />

<style>
	.actions {
		display: flex;
		justify-content: flex-end;
		margin-bottom: var(--sy-space-12);
	}
</style>
