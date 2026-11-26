<!-- Spec: docs/design/components/compare-screen.md -->
<script lang="ts">
	import type { CategoryName } from '../api/index.ts';
	import Button from '../components/Button/Button.svelte';
	import CategoryChip from '../components/CategoryChip/CategoryChip.svelte';
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
			<button
				type="button"
				class="chip"
				aria-pressed={selected.includes(info.name)}
				onclick={() => {
					toggle(info.name);
				}}
			>
				<CategoryChip category={info.name} />
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
		<Button size="sm" variant="secondary" type="submit">Apply</Button>
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
	{#if session.sides.length >= 3}
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
		flex-wrap: wrap;
		gap: var(--sy-space-4);
	}

	.chip {
		padding: 0;
		border: var(--sy-border-width-strong) solid transparent;
		border-radius: var(--sy-control-radius);
		background: none;
		cursor: pointer;
	}

	.chip[aria-pressed='true'] {
		border-color: var(--sy-color-accent-default);
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
