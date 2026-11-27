<!-- Spec: docs/design/components/checkbox.md -->
<script lang="ts">
	interface Props {
		label: string;
		/** One line under the label saying what the option does. */
		description?: string;
		checked?: boolean;
		disabled?: boolean;
		class?: string;
	}

	let {
		label,
		description,
		checked = $bindable(false),
		disabled = false,
		class: className
	}: Props = $props();
	const id = $props.id();
</script>

{#snippet box()}
	<label class={['sy-checkbox', className]} data-disabled={disabled || undefined}>
		<input
			type="checkbox"
			bind:checked
			{disabled}
			aria-describedby={description ? `${id}-description` : undefined}
		/>
		<span>{label}</span>
	</label>
{/snippet}

{#if description}
	<!-- Outside the label, so the description stays out of the checkbox's name. -->
	<div class="described">
		{@render box()}
		<span class="description" id="{id}-description">{description}</span>
	</div>
{:else}
	{@render box()}
{/if}

<style>
	.sy-checkbox {
		display: inline-flex;
		align-items: center;
		gap: var(--sy-space-8);
		min-height: var(--sy-control-height-sm);
		cursor: pointer;
	}

	input {
		width: var(--sy-icon-size-md);
		height: var(--sy-icon-size-md);
		margin: 0;
		accent-color: var(--sy-color-accent-default);
		cursor: inherit;
	}

	.described {
		display: grid;
	}

	/* Lines up with the label text, past the box. */
	.description {
		padding-left: calc(var(--sy-icon-size-md) + var(--sy-space-8));
		color: var(--sy-color-text-muted);
		font-size: var(--sy-font-size-small);
	}

	[data-disabled] {
		opacity: var(--sy-opacity-disabled);
		cursor: not-allowed;
	}

	@media (pointer: coarse) {
		.sy-checkbox {
			min-height: var(--sy-control-height-touch);
		}
	}
</style>
