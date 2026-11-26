<!-- Spec: docs/design/components/icon-button.md -->
<script lang="ts">
	import type { IconComponent } from '../icon';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	interface Props extends Omit<HTMLButtonAttributes, 'type' | 'children' | 'aria-label'> {
		/** Accessible name and tooltip; required because the button shows only an icon. */
		label: string;
		icon: IconComponent;
		variant?: 'secondary' | 'ghost' | 'danger';
		size?: 'sm' | 'md';
	}

	let {
		label,
		icon: Icon,
		variant = 'ghost',
		size = 'md',
		class: className,
		...rest
	}: Props = $props();
</script>

<button
	{...rest}
	type="button"
	class={['sy-icon-button', className]}
	data-variant={variant}
	data-size={size}
	aria-label={label}
	title={label}
>
	<Icon aria-hidden="true" />
</button>

<style>
	.sy-icon-button {
		display: inline-grid;
		place-items: center;
		width: var(--sy-control-height-md);
		height: var(--sy-control-height-md);
		padding: 0;
		border: var(--sy-border-width-default) solid transparent;
		border-radius: var(--sy-control-radius);
		background: transparent;
		color: var(--sy-color-text-secondary);
		cursor: pointer;
		transition: background-color var(--sy-motion-duration-state) var(--sy-motion-easing-enter);
	}

	.sy-icon-button :global(svg) {
		width: var(--sy-icon-size-md);
		height: var(--sy-icon-size-md);
	}

	.sy-icon-button[data-size='sm'] {
		width: var(--sy-control-height-sm);
		height: var(--sy-control-height-sm);
	}

	.sy-icon-button[data-size='sm'] :global(svg) {
		width: var(--sy-icon-size-sm);
		height: var(--sy-icon-size-sm);
	}

	.sy-icon-button[data-variant='secondary'] {
		border-color: var(--sy-color-border-strong);
		background: var(--sy-color-surface-base);
	}

	.sy-icon-button[data-variant='danger'] {
		color: var(--sy-status-error-fg);
	}

	.sy-icon-button:hover:not(:disabled) {
		background: var(--sy-color-surface-hover);
		color: var(--sy-color-text-primary);
	}

	.sy-icon-button:active:not(:disabled) {
		border-color: var(--sy-color-text-muted);
		background: var(--sy-color-surface-active);
	}

	.sy-icon-button:disabled {
		opacity: var(--sy-opacity-disabled);
		cursor: not-allowed;
	}

	@media (pointer: coarse) {
		.sy-icon-button {
			min-width: var(--sy-control-height-touch);
			min-height: var(--sy-control-height-touch);
		}
	}
</style>
