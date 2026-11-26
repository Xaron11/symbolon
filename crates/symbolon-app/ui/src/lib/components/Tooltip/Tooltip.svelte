<!-- Spec: docs/design/components/tooltip.md -->
<script lang="ts">
	import { Tooltip } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import '../overlay.css';

	interface Props {
		text: string;
		/** Renders the trigger; spread `props` onto the focusable element. */
		children: Snippet<[Record<string, unknown>]>;
	}

	let { text, children }: Props = $props();
</script>

<Tooltip.Provider delayDuration={400} skipDelayDuration={0}>
	<Tooltip.Root>
		<Tooltip.Trigger>
			{#snippet child({ props })}{@render children(props)}{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Content class="sy-floating sy-tooltip" role="tooltip" sideOffset={6}>
				{text}
			</Tooltip.Content>
		</Tooltip.Portal>
	</Tooltip.Root>
</Tooltip.Provider>

<style>
	:global(.sy-tooltip) {
		min-width: 0;
		padding: var(--sy-space-2) var(--sy-space-8);
		font-size: var(--sy-font-size-small);
		line-height: var(--sy-font-line-small);
	}
</style>
