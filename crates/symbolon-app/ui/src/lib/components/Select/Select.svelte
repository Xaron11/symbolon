<!-- Spec: docs/design/components/select.md -->
<script lang="ts" generics="T extends string">
	import { ChevronDown } from '@lucide/svelte';
	import { Select } from 'bits-ui';
	import '../overlay.css';

	interface Props {
		label: string;
		value: T;
		options: readonly { value: T; label: string }[];
		disabled?: boolean;
	}

	let { label, value = $bindable(), options, disabled = false }: Props = $props();
	const id = $props.id();
	const selected = $derived(options.find((o) => o.value === value)?.label ?? '');
</script>

<div class="sy-select">
	<span id="{id}-label" class="label">{label}</span>
	<Select.Root
		type="single"
		bind:value={() => value, (v) => (value = v as T)}
		items={options.map((o) => ({ value: o.value, label: o.label }))}
		{disabled}
	>
		<Select.Trigger class="sy-select-trigger" aria-labelledby="{id}-label">
			<span>{selected}</span>
			<ChevronDown aria-hidden="true" />
		</Select.Trigger>
		<Select.Portal>
			<Select.Content class="sy-floating" sideOffset={4}>
				<Select.Viewport>
					{#each options as option (option.value)}
						<Select.Item class="sy-option" value={option.value} label={option.label}>
							{option.label}
						</Select.Item>
					{/each}
				</Select.Viewport>
			</Select.Content>
		</Select.Portal>
	</Select.Root>
</div>

<style>
	.sy-select {
		display: grid;
		gap: var(--sy-space-4);
	}

	.label {
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		line-height: var(--sy-font-line-small);
	}

	:global(.sy-select-trigger) {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--sy-space-8);
		height: var(--sy-control-height-md);
		padding: 0 var(--sy-space-8);
		border: var(--sy-border-width-default) solid var(--sy-color-border-strong);
		border-radius: var(--sy-control-radius);
		background: var(--sy-color-surface-base);
		color: var(--sy-color-text-primary);
		font: inherit;
		cursor: pointer;
	}

	:global(.sy-select-trigger svg) {
		width: var(--sy-icon-size-sm);
		height: var(--sy-icon-size-sm);
		color: var(--sy-color-text-muted);
	}

	:global(.sy-select-trigger:disabled) {
		opacity: var(--sy-opacity-disabled);
		cursor: not-allowed;
	}

	@media (pointer: coarse) {
		:global(.sy-select-trigger) {
			min-height: var(--sy-control-height-touch);
		}
	}
</style>
