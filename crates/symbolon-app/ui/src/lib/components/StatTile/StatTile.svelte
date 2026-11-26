<!-- Spec: docs/design/components/stat-tile.md -->
<script lang="ts">
	import { category as info, type CategoryName } from '../../compare/categories';

	interface Props {
		label: string;
		value: string;
		detail?: string;
		/** Marks the tile with a category's color. */
		category?: CategoryName;
		class?: string;
	}

	let { label, value, detail, category, class: className }: Props = $props();
	const accent = $derived(
		category === undefined
			? 'var(--sy-color-border-strong)'
			: `var(--sy-category-${info(category).tokenKey}-fg)`
	);
</script>

<div class={['sy-stat', className]} style:--stat-accent={accent}>
	<span class="label">{label}</span>
	<span class="value sy-mono">{value}</span>
	{#if detail}<span class="detail sy-mono">{detail}</span>{/if}
</div>

<style>
	.sy-stat {
		display: grid;
		gap: var(--sy-space-2);
		padding: var(--sy-space-12);
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-left: var(--sy-border-width-strong) solid var(--stat-accent);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-base);
	}

	.label {
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		line-height: var(--sy-font-line-small);
	}

	.value {
		font-size: var(--sy-font-size-heading);
		font-weight: var(--sy-font-weight-semibold);
		line-height: var(--sy-font-line-heading);
	}

	.detail {
		color: var(--sy-color-text-muted);
		font-size: var(--sy-font-size-small);
	}
</style>
