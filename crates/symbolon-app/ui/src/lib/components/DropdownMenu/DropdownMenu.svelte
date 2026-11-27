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
		/** `sm` matches small buttons in table rows. */
		size?: 'sm' | 'md';
	}

	let { label, trigger, items, size = 'md' }: Props = $props();
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger class="sy-menu-trigger" data-size={size} aria-label={label}>
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
