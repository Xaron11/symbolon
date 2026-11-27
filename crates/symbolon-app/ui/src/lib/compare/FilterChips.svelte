<!-- Spec: docs/design/screens/compare-screen.md -->
<script lang="ts">
	import type { CategoryName } from '../api/index.ts';
	import { Check } from '@lucide/svelte';
	import Button from '../components/Button/Button.svelte';
	import Checkbox from '../components/Checkbox/Checkbox.svelte';
	import Input from '../components/Input/Input.svelte';
	import Select from '../components/Select/Select.svelte';
	import { CATEGORIES } from './categories';
	import type { CompareSession } from './session.svelte';

	let { session }: { session: CompareSession } = $props();

	const present = $derived(
		CATEGORIES.filter((info) =>
			session.summary?.categories.some((t) => t.category === info.name && t.rows > 0)
		)
	);
	const selected = $derived(session.filter.categories ?? []);
	let extension = $state('');
	let size = $state('any');
	const SIZES: Record<string, { minSize?: number; maxSize?: number }> = {
		any: {},
		small: { maxSize: 1024 ** 2 - 1 },
		medium: { minSize: 1024 ** 2, maxSize: 100 * 1024 ** 2 },
		large: { minSize: 100 * 1024 ** 2 + 1 }
	};

	function toggle(name: CategoryName) {
		const next = selected.includes(name) ? selected.filter((c) => c !== name) : [...selected, name];
		session.filter = { ...session.filter, categories: next };
	}

	function applyExtension() {
		const trimmed = extension.trim().replace(/^\./, '');
		session.filter = { ...session.filter, extension: trimmed === '' ? null : trimmed };
	}

	function applySize(next: string) {
		size = next;
		const range = SIZES[next] ?? {};
		session.filter = {
			...session.filter,
			minSize: range.minSize ?? null,
			maxSize: range.maxSize ?? null
		};
	}

	function togglePresence(side: number, on: boolean) {
		const current = session.filter.presence ?? [];
		const next = on ? [...current, side].sort((a, b) => a - b) : current.filter((s) => s !== side);
		session.filter = { ...session.filter, presence: next.length > 0 ? next : null };
	}
</script>

<div class="sy-filters" role="group" aria-label="Filters">
	<div class="chips">
		{#each present as info (info.name)}
			{@const on = selected.includes(info.name)}
			{@const Icon = on ? Check : info.icon}
			<button
				type="button"
				class="chip"
				aria-pressed={on}
				style:--chip-icon="var(--sy-category-{info.tokenKey}-fg)"
				onclick={() => {
					toggle(info.name);
				}}
			>
				<Icon aria-hidden="true" />{info.label}
			</button>
		{/each}
	</div>
	<form
		class="extension"
		onsubmit={(event) => {
			event.preventDefault();
			applyExtension();
		}}
	>
		<Input label="Extension" placeholder="jpg" bind:value={extension} />
		<Button variant="secondary" type="submit">Apply</Button>
	</form>
	<Select
		label="Size"
		bind:value={() => size, applySize}
		options={[
			{ value: 'any', label: 'Any size' },
			{ value: 'small', label: 'Under 1 MiB' },
			{ value: 'medium', label: '1–100 MiB' },
			{ value: 'large', label: 'Over 100 MiB' }
		]}
	/>
	{#if session.sides.length >= 2}
		<fieldset class="presence" aria-label="Present on">
			<legend>Present on</legend>
			{#each session.sides as side, i (i)}
				<Checkbox
					label={side.label}
					bind:checked={
						() => (session.filter.presence ?? []).includes(i),
						(on: boolean) => {
							togglePresence(i, on);
						}
					}
				/>
			{/each}
		</fieldset>
	{/if}
</div>

<style>
	.sy-filters {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		gap: var(--sy-space-12);
	}

	.chips {
		display: flex;
		flex-basis: 100%;
		flex-wrap: wrap;
		gap: var(--sy-space-4);
	}

	/* Toggle buttons: outlined and hoverable, unlike the flat category labels elsewhere. */
	.chip {
		display: inline-flex;
		align-items: center;
		gap: var(--sy-space-4);
		height: var(--sy-control-height-sm);
		padding: 0 var(--sy-space-8);
		border: var(--sy-border-width-default) solid var(--sy-color-border-strong);
		border-radius: var(--sy-control-radius);
		background: var(--sy-color-surface-base);
		color: var(--sy-color-text-secondary);
		font: inherit;
		font-size: var(--sy-font-size-small);
		cursor: pointer;
	}

	.chip :global(svg) {
		width: var(--sy-icon-size-sm);
		height: var(--sy-icon-size-sm);
		color: var(--chip-icon);
	}

	.chip:hover {
		background: var(--sy-color-surface-hover);
		color: var(--sy-color-text-primary);
	}

	.chip[aria-pressed='true'] {
		border-color: var(--sy-color-accent-default);
		background: var(--sy-color-surface-selected);
		color: var(--sy-color-text-primary);
	}

	.chip[aria-pressed='true'] :global(svg) {
		color: var(--sy-color-accent-default);
	}

	.extension {
		display: flex;
		align-items: flex-end;
		gap: var(--sy-space-4);
	}

	.presence {
		display: flex;
		gap: var(--sy-space-8);
		margin: 0;
		padding: 0;
		border: 0;
	}

	legend {
		margin-bottom: var(--sy-space-4);
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
	}
</style>
