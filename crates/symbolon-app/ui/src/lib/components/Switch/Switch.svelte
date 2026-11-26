<!-- Spec: docs/design/components/switch.md -->
<script lang="ts">
	interface Props {
		label: string;
		checked?: boolean;
		disabled?: boolean;
		class?: string;
	}

	let { label, checked = $bindable(false), disabled = false, class: className }: Props = $props();
</script>

<button
	type="button"
	role="switch"
	class={['sy-switch', className]}
	aria-checked={checked}
	{disabled}
	onclick={() => (checked = !checked)}
>
	<span class="track" aria-hidden="true"><span class="thumb"></span></span>
	<span>{label}</span>
</button>

<style>
	.sy-switch {
		display: inline-flex;
		align-items: center;
		gap: var(--sy-space-8);
		min-height: var(--sy-control-height-sm);
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--sy-color-text-primary);
		font: inherit;
		cursor: pointer;
	}

	.track {
		display: inline-flex;
		width: calc(var(--sy-icon-size-md) * 2);
		height: var(--sy-icon-size-md);
		padding: var(--sy-space-2);
		border: var(--sy-border-width-default) solid var(--sy-color-border-strong);
		border-radius: var(--sy-radius-full);
		background: var(--sy-color-surface-sunken);
		transition: background-color var(--sy-motion-duration-state) var(--sy-motion-easing-enter);
	}

	.thumb {
		width: calc(var(--sy-icon-size-md) - var(--sy-space-4) - var(--sy-border-width-default) * 2);
		height: 100%;
		border-radius: var(--sy-radius-full);
		background: var(--sy-color-text-muted);
		transition: transform var(--sy-motion-duration-state) var(--sy-motion-easing-enter);
	}

	[aria-checked='true'] .track {
		border-color: var(--sy-color-accent-default);
		background: var(--sy-color-accent-default);
	}

	[aria-checked='true'] .thumb {
		background: var(--sy-color-text-on-accent);
		transform: translateX(var(--sy-icon-size-md));
	}

	.sy-switch:disabled {
		opacity: var(--sy-opacity-disabled);
		cursor: not-allowed;
	}

	@media (pointer: coarse) {
		.sy-switch {
			min-height: var(--sy-control-height-touch);
		}
	}
</style>
