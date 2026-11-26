<!-- Spec: docs/design/components/dropdown-menu.md -->
<script lang="ts">
	import { DropdownMenu } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import type { MenuItem } from '../menu';
	import '../overlay.css';

	interface Props {
		/** Accessible name of the trigger, e.g. "Location actions". */
		label: string;
		trigger: Snippet;
		items: readonly MenuItem[];
	}

	let { label, trigger, items }: Props = $props();
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger class="sy-menu-trigger" aria-label={label}>
		{@render trigger()}
	</DropdownMenu.Trigger>
	<DropdownMenu.Portal>
		<DropdownMenu.Content class="sy-floating" sideOffset={4} align="start">
			{#each items as item, i (i)}
				{#if 'separator' in item}
					<DropdownMenu.Separator class="sy-separator" />
				{:else}
					{@const Icon = item.icon}
					<DropdownMenu.Item
						class="sy-option"
						disabled={item.disabled ?? false}
						onSelect={item.onSelect}
						data-danger={item.danger === true ? '' : undefined}
					>
						{#if Icon}<Icon aria-hidden="true" />{/if}
						<span>{item.label}</span>
						{#if item.shortcut}<span class="shortcut">{item.shortcut}</span>{/if}
					</DropdownMenu.Item>
				{/if}
			{/each}
		</DropdownMenu.Content>
	</DropdownMenu.Portal>
</DropdownMenu.Root>

<style>
	:global(.sy-menu-trigger) {
		display: inline-flex;
		align-items: center;
		gap: var(--sy-space-8);
		height: var(--sy-control-height-md);
		padding: 0 var(--sy-space-12);
		border: var(--sy-border-width-default) solid var(--sy-color-border-strong);
		border-radius: var(--sy-control-radius);
		background: var(--sy-color-surface-base);
		color: var(--sy-color-text-primary);
		font: inherit;
		cursor: pointer;
	}

	:global(.sy-menu-trigger:hover) {
		background: var(--sy-color-surface-hover);
	}

	@media (pointer: coarse) {
		:global(.sy-menu-trigger) {
			min-height: var(--sy-control-height-touch);
		}
	}
</style>
