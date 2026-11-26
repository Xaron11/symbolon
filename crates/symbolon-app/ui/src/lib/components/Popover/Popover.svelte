<!-- Spec: docs/design/components/popover.md -->
<script lang="ts">
	import { Popover } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import '../overlay.css';

	interface Props {
		open?: boolean;
		trigger: Snippet;
		children: Snippet;
	}

	let { open = $bindable(false), trigger, children }: Props = $props();
</script>

<Popover.Root bind:open>
	<Popover.Trigger class="sy-menu-trigger">{@render trigger()}</Popover.Trigger>
	<Popover.Portal>
		<Popover.Content class="sy-floating sy-popover" sideOffset={4}>
			{@render children()}
		</Popover.Content>
	</Popover.Portal>
</Popover.Root>

<style>
	:global(.sy-popover) {
		max-width: calc(var(--sy-control-height-md) * 10);
		padding: var(--sy-space-12);
	}
</style>
