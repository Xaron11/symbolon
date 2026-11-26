<!-- Spec: docs/design/components/app-shell.md -->
<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		/** Activity, such as running scans. */
		children: Snippet;
		/** Context, such as the catalog path. */
		end?: Snippet;
	}

	let { children, end }: Props = $props();
</script>

<div class="sy-statusbar">
	<div>{@render children()}</div>
	{#if end}<div class="end sy-mono">{@render end()}</div>{/if}
</div>

<style>
	.sy-statusbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--sy-space-16);
		height: 100%;
		padding: 0 var(--sy-space-12);
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		line-height: var(--sy-font-line-small);
	}

	.end {
		color: var(--sy-color-text-muted);
	}
</style>
