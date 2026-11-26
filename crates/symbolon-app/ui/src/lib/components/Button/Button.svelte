<!-- Spec: docs/design/components/button.md -->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	interface Props extends Omit<HTMLButtonAttributes, 'type' | 'children'> {
		variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
		size?: 'sm' | 'md';
		type?: 'button' | 'submit';
		loading?: boolean;
		children: Snippet;
	}

	let {
		variant = 'secondary',
		size = 'md',
		type = 'button',
		loading = false,
		disabled = false,
		class: className,
		onclick,
		children,
		...rest
	}: Props = $props();
</script>

<button
	{...rest}
	{type}
	class={['sy-button', className]}
	data-variant={variant}
	data-size={size}
	disabled={disabled ?? false}
	aria-busy={loading || undefined}
	aria-disabled={loading || undefined}
	onclick={(event) => {
		// While loading the button keeps focus, so it only ignores activation.
		if (!loading) onclick?.(event);
	}}
>
	{#if loading}<span class="spinner" aria-hidden="true"></span>{/if}
	{@render children()}
</button>

<style>
	.sy-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--sy-space-8);
		height: var(--sy-control-height-md);
		padding: 0 var(--sy-space-12);
		border: var(--sy-border-width-default) solid var(--sy-color-border-strong);
		border-radius: var(--sy-control-radius);
		background: var(--sy-color-surface-base);
		color: var(--sy-color-text-primary);
		font: inherit;
		font-weight: var(--sy-font-weight-medium);
		white-space: nowrap;
		cursor: pointer;
		transition:
			background-color var(--sy-motion-duration-state) var(--sy-motion-easing-enter),
			border-color var(--sy-motion-duration-state) var(--sy-motion-easing-enter);
	}

	.sy-button[data-size='sm'] {
		height: var(--sy-control-height-sm);
		padding: 0 var(--sy-space-8);
	}

	.sy-button:hover:not(:disabled) {
		background: var(--sy-color-surface-hover);
	}

	/* Pressed: fill and border change, since the dark theme's pressed fill equals its hover. */
	.sy-button:active:not(:disabled) {
		background: var(--sy-color-surface-active);
		border-color: var(--sy-color-text-muted);
	}

	.sy-button[data-variant='primary'] {
		border-color: var(--sy-color-accent-default);
		background: var(--sy-color-accent-default);
		color: var(--sy-color-text-on-accent);
	}

	.sy-button[data-variant='primary']:hover:not(:disabled) {
		border-color: var(--sy-color-accent-hover);
		background: var(--sy-color-accent-hover);
	}

	.sy-button[data-variant='primary']:active:not(:disabled) {
		border-color: var(--sy-color-accent-active);
		background: var(--sy-color-accent-active);
	}

	.sy-button[data-variant='ghost'] {
		border-color: transparent;
		background: transparent;
	}

	.sy-button[data-variant='danger'] {
		border-color: var(--sy-status-error-fg);
		background: var(--sy-status-error-fg);
		color: var(--sy-color-surface-base);
	}

	.sy-button[data-variant='danger']:hover:not(:disabled),
	.sy-button[data-variant='danger']:active:not(:disabled) {
		border-color: var(--sy-color-text-primary);
		background: var(--sy-status-error-fg);
	}

	.sy-button:disabled {
		opacity: var(--sy-opacity-disabled);
		cursor: not-allowed;
	}

	.sy-button[aria-busy='true'] {
		cursor: progress;
	}

	.spinner {
		width: var(--sy-icon-size-sm);
		height: var(--sy-icon-size-sm);
		border: var(--sy-border-width-strong) solid currentColor;
		border-right-color: transparent;
		border-radius: var(--sy-radius-full);
		animation: spin calc(var(--sy-motion-duration-enter) * 5) linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	@media (pointer: coarse) {
		.sy-button {
			min-height: var(--sy-control-height-touch);
		}
	}
</style>
