<!-- Spec: docs/design/components/tabs.md -->
<script lang="ts" generics="T extends string">
	import { Tabs } from 'bits-ui';
	import type { Snippet } from 'svelte';

	interface Props {
		/** Accessible name of the tab list. */
		label: string;
		value: T;
		tabs: readonly { value: T; label: string; content: Snippet }[];
	}

	let { label, value = $bindable(), tabs }: Props = $props();
</script>

<Tabs.Root
	bind:value={
		() => value,
		(v) => {
			if (v) value = v;
		}
	}
	class="sy-tabs"
>
	<Tabs.List class="sy-tab-list" aria-label={label}>
		{#each tabs as tab (tab.value)}
			<Tabs.Trigger class="sy-tab" value={tab.value}>{tab.label}</Tabs.Trigger>
		{/each}
	</Tabs.List>
	{#each tabs as tab (tab.value)}
		<Tabs.Content class="sy-tab-panel" value={tab.value}>{@render tab.content()}</Tabs.Content>
	{/each}
</Tabs.Root>

<style>
	:global(.sy-tab-list) {
		display: flex;
		gap: var(--sy-space-4);
		border-bottom: var(--sy-border-width-default) solid var(--sy-color-border-default);
	}

	:global(.sy-tab) {
		height: var(--sy-control-height-md);
		padding: 0 var(--sy-space-12);
		border: 0;
		border-bottom: var(--sy-border-width-strong) solid transparent;
		background: transparent;
		color: var(--sy-color-text-secondary);
		font: inherit;
		cursor: pointer;
	}

	:global(.sy-tab:hover) {
		color: var(--sy-color-text-primary);
	}

	:global(.sy-tab[data-state='active']) {
		border-bottom-color: var(--sy-color-accent-default);
		color: var(--sy-color-text-primary);
		font-weight: var(--sy-font-weight-medium);
	}

	:global(.sy-tab-panel) {
		padding-top: var(--sy-space-12);
	}

	@media (pointer: coarse) {
		:global(.sy-tab) {
			min-height: var(--sy-control-height-touch);
		}
	}
</style>
