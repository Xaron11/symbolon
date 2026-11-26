<!-- Spec: docs/design/components/add-location-dialog.md -->
<script lang="ts">
	import { backend, toApiError } from '../api/index.ts';
	import Button from '../components/Button/Button.svelte';
	import Dialog from '../components/Dialog/Dialog.svelte';
	import Input from '../components/Input/Input.svelte';
	import { explain } from './status';

	interface Props {
		/** The location to rename; the dialog is open while it is set. */
		label?: string | undefined;
		onrenamed: () => void;
	}

	let { label = $bindable(), onrenamed }: Props = $props();
	const api = backend();
	const formId = $props.id();
	let next = $state('');
	let error = $state<string | undefined>(undefined);

	$effect(() => {
		if (label !== undefined) {
			next = label;
			error = undefined;
		}
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (label === undefined) return;
		if (next.trim() === '') {
			error = 'Enter a label.';
			return;
		}
		try {
			await api.locations.rename(label, next.trim());
			label = undefined;
			onrenamed();
		} catch (e) {
			error = explain(toApiError(e));
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
	title="Rename {label ?? ''}"
>
	<form id={formId} onsubmit={submit} novalidate>
		<Input label="New label" bind:value={next} {...error ? { error } : {}} />
	</form>
	{#snippet footer()}
		<Button
			variant="ghost"
			onclick={() => {
				label = undefined;
			}}
		>
			Cancel
		</Button>
		<Button variant="primary" type="submit" form={formId}>Rename</Button>
	{/snippet}
</Dialog>
