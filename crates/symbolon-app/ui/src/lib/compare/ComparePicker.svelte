<!-- Spec: docs/design/components/compare-screen.md -->
<script lang="ts">
	import { SvelteSet } from 'svelte/reactivity';
	import { backend, toApiError, type CompareRequest, type LocationView } from '../api/index.ts';
	import Alert from '../components/Alert/Alert.svelte';
	import Button from '../components/Button/Button.svelte';
	import Checkbox from '../components/Checkbox/Checkbox.svelte';
	import EmptyState from '../components/EmptyState/EmptyState.svelte';
	import Input from '../components/Input/Input.svelte';
	import { formatAgo, formatBytes } from '../format';
	import { explain } from '../locations/status';
	import { openComparison } from './open.svelte';
	import { toQuery } from './url';

	interface Props {
		/** Opens a route: the comparison once chosen. */
		navigate: (href: string, options?: { replace: boolean }) => void;
	}

	let { navigate }: Props = $props();
	const api = backend();
	const open = openComparison();
	// Compare shows the comparison that is open; "New comparison" there closes it first.
	// svelte-ignore state_referenced_locally
	if (open.href) navigate(open.href, { replace: true });
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
	let toleranceError = $state<string | undefined>(undefined);

	api.locations.list().then(
		(list) => (locations = list),
		(e: unknown) => (loadError = explain(toApiError(e)))
	);

	const count = new Intl.NumberFormat('en-US');
	const chosenLabels = $derived(
		(locations ?? []).filter((l) => chosen.has(l.label)).map((l) => l.label)
	);
	const summary = $derived(
		chosenLabels.length === 0
			? 'No locations chosen'
			: chosenLabels.length === 1
				? `${chosenLabels[0] ?? ''} only`
				: `${chosenLabels.slice(0, -1).join(', ')} and ${chosenLabels.at(-1) ?? ''}`
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
		// People write "1,5" as often as "1.5".
		const seconds = Number(fastSeconds.trim().replace(',', '.'));
		if (fast && (fastSeconds.trim() === '' || !Number.isFinite(seconds) || seconds < 0)) {
			toleranceError = 'Enter a tolerance of 0 seconds or more.';
			return;
		}
		toleranceError = undefined;
		const request: CompareRequest = {
			sides: order.map((l) => {
				const subpath = (subpaths[l.label] ?? '').trim();
				return subpath === '' ? { label: l.label } : { label: l.label, subpath };
			}),
			nfc,
			caseInsensitive,
			...(fast && { fastMs: Math.round(seconds * 1000) }),
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
	<div class="wrap">
		<form
			class="picker"
			onsubmit={(event) => {
				event.preventDefault();
				compare();
			}}
		>
			<fieldset class="locations">
				<legend>Locations to compare</legend>
				<p class="hint">Choose two or more. Each one's latest scan is compared.</p>
				<div class="cards">
					{#each locations as location (location.label)}
						{@const on = chosen.has(location.label)}
						{@const last = location.lastScan}
						<!-- The checkbox is the keyboard target; the card only widens it for pointers. -->
						<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
						<div
							class="card"
							class:on
							onclick={(event) => {
								const target = event.target as Element;
								if (!target.closest('label, input')) toggle(location.label, !on);
							}}
						>
							<Checkbox
								label={location.label}
								bind:checked={
									() => on,
									(next: boolean) => {
										toggle(location.label, next);
									}
								}
							/>
							<p class="root sy-mono" title={location.root}>{location.root}</p>
							<p class="facts">
								{#if last}
									{count.format(location.stats.files)} files · {formatBytes(location.stats.bytes)} · scanned
									{formatAgo(last.finishedMs ?? last.startedMs)}
								{:else}
									Not scanned yet
								{/if}
							</p>
							{#if on}
								<Input
									label="Only this subfolder (optional)"
									placeholder="e.g. Photos/2024"
									bind:value={
										() => subpaths[location.label] ?? '',
										(value: string) => (subpaths = { ...subpaths, [location.label]: value })
									}
								/>
							{/if}
						</div>
					{/each}
				</div>
			</fieldset>
			<div class="options-panel">
				<fieldset class="options">
					<legend>Comparison options</legend>
					<Checkbox
						label="Ignore case"
						description="Photo.JPG and photo.jpg count as the same file."
						bind:checked={caseInsensitive}
					/>
					<Checkbox
						label="Unicode normalization (NFC)"
						description="Names typed differently on macOS and Linux still match."
						bind:checked={nfc}
					/>
					<Checkbox
						label="Fast mode: assume files equal by size and time"
						description="Skips hashing; quicker, but can miss edited files."
						bind:checked={fast}
					/>
					{#if fast}
						<Input
							label="Time tolerance in seconds"
							bind:value={fastSeconds}
							{...toleranceError ? { error: toleranceError } : {}}
						/>
					{/if}
					<Checkbox
						label="Find duplicates within each location"
						description="Also lists files with the same content inside one location."
						bind:checked={duplicates}
					/>
				</fieldset>
			</div>
			<div class="submit">
				{#if mismatch}<Alert tone="warning" title="Different hash algorithms">
						{mismatch}
					</Alert>{/if}
				{#if problem}<p class="problem" role="alert">{problem}</p>{/if}
				<div class="bar">
					<p class="summary">{summary}</p>
					<Button variant="primary" type="submit">Compare</Button>
				</div>
			</div>
		</form>
	</div>
{/if}

<style>
	/* Locations take the room; options sit beside them on wide windows and below on narrow ones. */
	.picker {
		display: grid;
		grid-template-columns: minmax(0, 1fr) var(--sy-picker-options-width);
		grid-template-areas:
			'locations options'
			'submit submit';
		gap: var(--sy-space-24);
		align-items: start;
	}

	.wrap {
		container-type: inline-size;
	}

	@container (width < 720px) {
		.picker {
			grid-template-columns: minmax(0, 1fr);
			grid-template-areas: 'locations' 'options' 'submit';
		}
	}

	fieldset {
		min-width: 0;
		margin: 0;
		padding: 0;
		border: 0;
	}

	legend {
		padding: 0;
		font-size: var(--sy-font-size-title);
		font-weight: var(--sy-font-weight-semibold);
	}

	.locations {
		grid-area: locations;
	}

	.hint {
		margin: var(--sy-space-4) 0 var(--sy-space-12);
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
	}

	.cards {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(var(--sy-scan-card-min-width), 1fr));
		gap: var(--sy-space-12);
	}

	.card {
		display: grid;
		gap: var(--sy-space-4);
		align-content: start;
		min-width: 0;
		padding: var(--sy-space-12);
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-base);
		cursor: pointer;
	}

	.card:hover {
		border-color: var(--sy-color-border-strong);
	}

	.card.on {
		border-color: var(--sy-color-accent-default);
		background: var(--sy-color-surface-selected);
	}

	.card :global(.sy-checkbox) {
		font-weight: var(--sy-font-weight-medium);
	}

	.root,
	.facts {
		margin: 0;
		overflow: hidden;
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.facts {
		color: var(--sy-color-text-muted);
	}

	.options-panel {
		grid-area: options;
		padding: var(--sy-space-16);
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-sunken);
	}

	.options {
		display: grid;
		gap: var(--sy-space-12);
	}

	.options legend {
		margin-bottom: var(--sy-space-4);
		font-size: var(--sy-font-size-body);
	}

	.submit {
		display: grid;
		grid-area: submit;
		gap: var(--sy-space-12);
	}

	.bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--sy-space-16);
		padding-top: var(--sy-space-16);
		border-top: var(--sy-border-width-default) solid var(--sy-color-border-default);
	}

	.summary {
		margin: 0;
		color: var(--sy-color-text-secondary);
	}

	.problem {
		margin: 0;
		color: var(--sy-status-error-fg);
	}
</style>
