<!-- Spec: docs/design/components/compare-screen.md -->
<script lang="ts">
	import { SvelteSet } from 'svelte/reactivity';
	import { backend, toApiError, type CompareRequest, type LocationView } from '../api/index.ts';
	import Alert from '../components/Alert/Alert.svelte';
	import Button from '../components/Button/Button.svelte';
	import Checkbox from '../components/Checkbox/Checkbox.svelte';
	import EmptyState from '../components/EmptyState/EmptyState.svelte';
	import Input from '../components/Input/Input.svelte';
	import { explain } from '../locations/status';
	import { toQuery } from './url';

	interface Props {
		/** Opens a route: the comparison once chosen. */
		navigate: (href: string) => void;
	}

	let { navigate }: Props = $props();
	const api = backend();
	let locations = $state<LocationView[] | undefined>(undefined);
	let loadError = $state<string | undefined>(undefined);
	const chosen = new SvelteSet<string>();
	let subpaths = $state<Record<string, string>>({});
	let caseInsensitive = $state(false);
	let nfc = $state(false);
	let fast = $state(false);
	let fastSeconds = $state('2');
	let duplicates = $state(false);
	let problem = $state<string | undefined>(undefined);

	api.locations.list().then(
		(list) => (locations = list),
		(e: unknown) => (loadError = explain(toApiError(e)))
	);

	const ALGORITHMS: Record<string, string> = { blake3: 'BLAKE3', xxh3: 'XXH3', sha256: 'SHA-256' };
	const named = (algo: string) => ALGORITHMS[algo] ?? algo;

	// Locations hashed with different algorithms cannot be compared (the bridge refuses too).
	const mismatch = $derived.by(() => {
		const picked = (locations ?? []).filter((l) => chosen.has(l.label));
		const first = picked[0];
		const other = picked.find((l) => l.hashAlgo !== first?.hashAlgo);
		return first && other
			? `${first.label} uses ${named(first.hashAlgo)} and ${other.label} uses ${named(other.hashAlgo)}; compare locations with the same algorithm.`
			: undefined;
	});

	function toggle(label: string, on: boolean) {
		if (on) chosen.add(label);
		else chosen.delete(label);
		problem = undefined;
	}

	function compare() {
		const order = (locations ?? []).filter((l) => chosen.has(l.label));
		if (order.length < (duplicates ? 1 : 2)) {
			problem = 'Choose at least two locations, or one with Find duplicates on.';
			return;
		}
		if (mismatch) return;
		const seconds = Number(fastSeconds);
		const request: CompareRequest = {
			sides: order.map((l) => {
				const subpath = (subpaths[l.label] ?? '').trim();
				return subpath === '' ? { label: l.label } : { label: l.label, subpath };
			}),
			nfc,
			caseInsensitive,
			...(fast && Number.isFinite(seconds) && { fastMs: Math.round(seconds * 1000) }),
			duplicates
		};
		navigate(`/compare/view?${toQuery(request).toString()}`);
	}
</script>

{#if loadError}
	<Alert tone="error" title="Could not read the locations">{loadError}</Alert>
{:else if locations?.length === 0}
	<EmptyState title="Nothing to compare yet" description="Add and scan locations first.">
		{#snippet actions()}<a href="/locations">Go to Locations</a>{/snippet}
	</EmptyState>
{:else if locations !== undefined}
	<form
		class="picker"
		onsubmit={(event) => {
			event.preventDefault();
			compare();
		}}
	>
		<fieldset>
			<legend>Locations</legend>
			{#each locations as location (location.label)}
				<div class="side">
					<Checkbox
						label={location.label}
						bind:checked={
							() => chosen.has(location.label),
							(on: boolean) => {
								toggle(location.label, on);
							}
						}
					/>
					{#if chosen.has(location.label)}
						<Input
							label="Subfolder of {location.label} (optional)"
							bind:value={
								() => subpaths[location.label] ?? '',
								(value: string) => (subpaths = { ...subpaths, [location.label]: value })
							}
						/>
					{/if}
				</div>
			{/each}
		</fieldset>
		<fieldset>
			<legend>Matching</legend>
			<Checkbox label="Ignore case" bind:checked={caseInsensitive} />
			<Checkbox label="Unicode normalization (NFC)" bind:checked={nfc} />
			<Checkbox label="Fast mode: assume files equal by size and time" bind:checked={fast} />
			{#if fast}
				<Input label="Time tolerance in seconds" bind:value={fastSeconds} />
			{/if}
			<Checkbox label="Find duplicates within each location" bind:checked={duplicates} />
		</fieldset>
		{#if mismatch}<Alert tone="warning" title="Different hash algorithms">{mismatch}</Alert>{/if}
		{#if problem}<p class="problem" role="alert">{problem}</p>{/if}
		<div><Button variant="primary" type="submit">Compare</Button></div>
	</form>
{/if}

<style>
	.picker {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-16);
		max-width: var(--sy-form-width);
	}

	fieldset {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-8);
		margin: 0;
		padding: 0;
		border: 0;
	}

	legend {
		margin-bottom: var(--sy-space-8);
		font-weight: var(--sy-font-weight-medium);
	}

	.side {
		display: flex;
		flex-direction: column;
		gap: var(--sy-space-4);
	}

	.problem {
		margin: 0;
		color: var(--sy-status-error-fg);
	}
</style>
