<!-- Spec: docs/design/components/badge.md -->
<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		tone?: 'neutral' | 'accent' | 'error' | 'warning' | 'success' | 'info';
		class?: string;
		children: Snippet;
	}

	let { tone = 'neutral', class: className, children }: Props = $props();
</script>

<span class={['sy-badge', className]} data-tone={tone}>{@render children()}</span>

<style>
	/* Badges state something; they are flat so they never look like buttons. */
	.sy-badge {
		display: inline-flex;
		align-items: center;
		gap: var(--sy-space-4);
		padding: 0 var(--sy-space-8);
		border-radius: var(--sy-radius-full);
		background: var(--sy-color-surface-sunken);
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		line-height: var(--sy-font-line-small);
		white-space: nowrap;
	}

	.sy-badge :global(svg) {
		flex: none;
		width: var(--sy-icon-size-sm);
		height: var(--sy-icon-size-sm);
	}

	[data-tone='accent'] {
		color: var(--sy-color-text-primary);
	}

	[data-tone='accent'] :global(svg) {
		color: var(--sy-color-accent-default);
	}

	[data-tone='error'] {
		background: var(--sy-status-error-bg);
		color: var(--sy-status-error-fg);
	}

	[data-tone='warning'] {
		background: var(--sy-status-warning-bg);
		color: var(--sy-status-warning-fg);
	}

	[data-tone='success'] {
		background: var(--sy-status-success-bg);
		color: var(--sy-status-success-fg);
	}

	[data-tone='info'] {
		background: var(--sy-status-info-bg);
		color: var(--sy-status-info-fg);
	}
</style>
