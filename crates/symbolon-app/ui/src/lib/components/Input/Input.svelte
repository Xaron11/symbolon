<!-- Spec: docs/design/components/input.md -->
<script lang="ts">
	interface Props {
		/** Always visible; never replaced by the placeholder. */
		label: string;
		value?: string;
		type?: 'text' | 'search';
		placeholder?: string;
		/** Shown under the field and linked to it with `aria-describedby`. */
		error?: string;
		/** A short help line under the field (replaced by `error` when there is one). */
		hint?: string;
		disabled?: boolean;
		class?: string;
	}

	let {
		label,
		value = $bindable(''),
		type = 'text',
		placeholder,
		error,
		hint,
		disabled = false,
		class: className
	}: Props = $props();
	const id = $props.id();
</script>

<div class={['sy-field', className]}>
	<label for={id}>{label}</label>
	<input
		{id}
		{type}
		bind:value
		{placeholder}
		{disabled}
		aria-invalid={error ? 'true' : undefined}
		aria-describedby={error ? `${id}-error` : hint ? `${id}-hint` : undefined}
	/>
	{#if error}<p id="{id}-error" class="error">{error}</p>
	{:else if hint}<p id="{id}-hint" class="hint">{hint}</p>{/if}
</div>

<style>
	.sy-field {
		display: grid;
		gap: var(--sy-space-4);
	}

	label {
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		line-height: var(--sy-font-line-small);
	}

	input {
		height: var(--sy-control-height-md);
		padding: 0 var(--sy-space-8);
		border: var(--sy-border-width-default) solid var(--sy-color-border-strong);
		border-radius: var(--sy-control-radius);
		background: var(--sy-color-surface-sunken);
		color: var(--sy-color-text-primary);
		font: inherit;
	}

	.hint {
		margin: 0;
		color: var(--sy-color-text-muted);
		font-size: var(--sy-font-size-small);
	}

	input::placeholder {
		color: var(--sy-color-text-muted);
	}

	input:focus-visible {
		border-color: var(--sy-color-focus);
	}

	input[aria-invalid='true'] {
		border-color: var(--sy-status-error-fg);
	}

	input:disabled {
		opacity: var(--sy-opacity-disabled);
		cursor: not-allowed;
	}

	.error {
		margin: 0;
		color: var(--sy-status-error-fg);
		font-size: var(--sy-font-size-small);
		line-height: var(--sy-font-line-small);
	}

	@media (pointer: coarse) {
		input {
			min-height: var(--sy-control-height-touch);
		}
	}
</style>
