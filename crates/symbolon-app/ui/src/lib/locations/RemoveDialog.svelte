<!-- Spec: docs/design/screens/add-location-dialog.md -->
<script lang="ts">
	import { backend, toApiError } from '../api/index.ts';
	import Alert from '../components/Alert/Alert.svelte';
	import Button from '../components/Button/Button.svelte';
	import Dialog from '../components/Dialog/Dialog.svelte';
	import { explain } from './status';

	interface Props {
		/** The location to remove; the dialog is open while it is set. */
		label?: string | undefined;
		onremoved: () => void;
	}

	let { label = $bindable(), onremoved }: Props = $props();
	const api = backend();
	let error = $state<string | undefined>(undefined);
	let busy = $state(false);

	$effect(() => {
		if (label !== undefined) error = undefined;
	});

	async function remove() {
		if (label === undefined) return;
		busy = true;
		try {
			await api.locations.remove(label);
			label = undefined;
			onremoved();
		} catch (e) {
			error = explain(toApiError(e));
		} finally {
			busy = false;
		}
	}
</script>

<Dialog
	bind:open={
		() => label !== undefined,
		(open) => {
			if (!open) label = undefined;
		}
	}
	title="Remove {label ?? ''}?"
>
	<p>Remove {label} from the catalog? Files on disk are not touched.</p>
	{#if error}<Alert tone="error" title="Could not remove it">{error}</Alert>{/if}
	{#snippet footer()}
		<Button
			variant="ghost"
			onclick={() => {
				label = undefined;
			}}
		>
			Keep {label}
		</Button>
		<Button
			variant="danger"
			loading={busy}
			onclick={() => {
				void remove();
			}}
		>
			Remove {label}
		</Button>
	{/snippet}
</Dialog>

<style>
	p {
		margin: 0 0 var(--sy-space-12);
	}
</style>
