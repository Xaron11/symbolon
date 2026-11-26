<!-- Spec: docs/design/components/combobox.md -->
<script lang="ts" generics="T extends string">
	import { Combobox } from 'bits-ui';
	import '../overlay.css';

	interface Option {
		value: T;
		label: string;
	}

	interface Props {
		label: string;
		/** The chosen option's value; undefined while nothing is chosen. */
		value?: T | undefined;
		options: readonly Option[];
		placeholder?: string;
		/** Whether `option` matches what was typed; default: label contains it, ignoring case. */
		filter?: (option: Option, query: string) => boolean;
	}

	let {
		label,
		value = $bindable(),
		options,
		placeholder,
		filter = (option, query) => option.label.toLowerCase().includes(query.toLowerCase())
	}: Props = $props();
	const id = $props.id();
	const labelOf = (v: T | undefined) => options.find((o) => o.value === v)?.label ?? '';
	// The field shows the chosen option's label, or what is being typed while the list is open.
	let text = $derived(labelOf(value));
	let query = $state('');
	const shown = $derived(query === '' ? options : options.filter((o) => filter(o, query)));
</script>

<div class="sy-combobox">
	<label for={id}>{label}</label>
	<Combobox.Root
		type="single"
		bind:value={() => value ?? '', (v) => (value = v === '' ? undefined : (v as T))}
		items={shown.map((o) => ({ value: o.value, label: o.label }))}
		inputValue={text}
		onOpenChange={(open) => {
			if (!open) {
				query = '';
				text = labelOf(value);
			}
		}}
	>
		<Combobox.Input
			{id}
			class="sy-combobox-input"
			{placeholder}
			oninput={(event) => {
				query = event.currentTarget.value;
				text = query;
			}}
		/>
		<Combobox.Portal>
			<Combobox.Content class="sy-floating" sideOffset={4}>
				<Combobox.Viewport>
					{#each shown as option (option.value)}
						<Combobox.Item class="sy-option" value={option.value} label={option.label}>
							{option.label}
						</Combobox.Item>
					{:else}
						<span class="sy-option empty">No matches</span>
					{/each}
				</Combobox.Viewport>
			</Combobox.Content>
		</Combobox.Portal>
	</Combobox.Root>
</div>

<style>
	.sy-combobox {
		display: grid;
		gap: var(--sy-space-4);
	}

	label {
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		line-height: var(--sy-font-line-small);
	}

	:global(.sy-combobox-input) {
		height: var(--sy-control-height-md);
		padding: 0 var(--sy-space-8);
		border: var(--sy-border-width-default) solid var(--sy-color-border-strong);
		border-radius: var(--sy-control-radius);
		background: var(--sy-color-surface-sunken);
		color: var(--sy-color-text-primary);
		font: inherit;
	}

	.empty {
		color: var(--sy-color-text-muted);
		cursor: default;
	}

	@media (pointer: coarse) {
		:global(.sy-combobox-input) {
			min-height: var(--sy-control-height-touch);
		}
	}
</style>
