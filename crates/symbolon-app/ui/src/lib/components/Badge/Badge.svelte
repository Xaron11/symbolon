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

<span class={['sy-badge', 'sy-mono', className]} data-tone={tone}>{@render children()}</span>

<style>
	.sy-badge {
		display: inline-flex;
		align-items: center;
		padding: 0 var(--sy-space-4);
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-radius-sm);
		background: var(--sy-color-surface-sunken);
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		line-height: var(--sy-font-line-small);
	}

	[data-tone='accent'] {
		border-color: var(--sy-color-accent-default);
		background: var(--sy-color-surface-selected);
		color: var(--sy-color-text-primary);
	}

	[data-tone='error'] {
		border-color: var(--sy-status-error-border);
		background: var(--sy-status-error-bg);
		color: var(--sy-status-error-fg);
	}

	[data-tone='warning'] {
		border-color: var(--sy-status-warning-border);
		background: var(--sy-status-warning-bg);
		color: var(--sy-status-warning-fg);
	}

	[data-tone='success'] {
		border-color: var(--sy-status-success-border);
		background: var(--sy-status-success-bg);
		color: var(--sy-status-success-fg);
	}

	[data-tone='info'] {
		border-color: var(--sy-status-info-border);
		background: var(--sy-status-info-bg);
		color: var(--sy-status-info-fg);
	}
</style>
