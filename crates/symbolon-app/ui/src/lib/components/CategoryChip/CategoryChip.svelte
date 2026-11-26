<!-- Spec: docs/design/components/category-chip.md -->
<script lang="ts">
	import { category as info, type CategoryName } from '../../compare/categories';

	interface Props {
		category: CategoryName;
		count?: number;
		/** Icon only; the label stays the accessible name. */
		compact?: boolean;
		class?: string;
	}

	let { category, count, compact = false, class: className }: Props = $props();
	const shown = $derived(info(category));
	const Icon = $derived(shown.icon);
	const format = new Intl.NumberFormat('en-US');
</script>

<span
	class={['sy-chip', className]}
	data-category={category}
	style:--chip-fg="var(--sy-category-{shown.tokenKey}-fg)"
	style:--chip-bg="var(--sy-category-{shown.tokenKey}-bg)"
	style:--chip-border="var(--sy-category-{shown.tokenKey}-border)"
	role={compact ? 'img' : undefined}
	aria-label={compact
		? count === undefined
			? shown.label
			: `${shown.label}: ${format.format(count)}`
		: undefined}
>
	<Icon aria-hidden="true" />
	{#if !compact}<span class="label">{shown.label}</span>{/if}
	{#if count !== undefined}<span class="count sy-mono">{format.format(count)}</span>{/if}
</span>

<style>
	.sy-chip {
		display: inline-flex;
		align-items: center;
		gap: var(--sy-space-4);
		padding: 0 var(--sy-space-8) 0 var(--sy-space-4);
		border: var(--sy-border-width-default) solid var(--chip-border);
		border-radius: var(--sy-radius-sm);
		background: var(--chip-bg);
		color: var(--sy-color-text-primary);
		font-size: var(--sy-font-size-small);
		line-height: var(--sy-font-line-small);
		white-space: nowrap;
	}

	.sy-chip :global(svg) {
		width: var(--sy-icon-size-sm);
		height: var(--sy-icon-size-sm);
		color: var(--chip-fg);
	}

	.count {
		color: var(--sy-color-text-secondary);
	}
</style>
