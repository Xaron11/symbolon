<!-- Spec: docs/design/components/context-menu.md -->
<script lang="ts">
	import { ContextMenu } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import type { MenuItem } from '../menu';
	import '../overlay.css';

	interface Props {
		items: readonly MenuItem[];
		/** The area that opens the menu on right click or the context-menu key. */
		children: Snippet;
	}

	let { items, children }: Props = $props();
</script>

<ContextMenu.Root>
	<ContextMenu.Trigger class="sy-context-target">{@render children()}</ContextMenu.Trigger>
	<ContextMenu.Portal>
		<ContextMenu.Content class="sy-floating">
			{#each items as item, i (i)}
				{#if 'separator' in item}
					<ContextMenu.Separator class="sy-separator" />
				{:else}
					{@const Icon = item.icon}
					<ContextMenu.Item
						class="sy-option"
						disabled={item.disabled ?? false}
						onSelect={item.onSelect}
						data-danger={item.danger === true ? '' : undefined}
					>
						{#if Icon}<Icon aria-hidden="true" />{/if}
						<span>{item.label}</span>
						{#if item.shortcut}<span class="shortcut">{item.shortcut}</span>{/if}
					</ContextMenu.Item>
				{/if}
			{/each}
		</ContextMenu.Content>
	</ContextMenu.Portal>
</ContextMenu.Root>
