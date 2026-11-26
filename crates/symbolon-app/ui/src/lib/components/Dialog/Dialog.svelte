<!-- Spec: docs/design/components/dialog.md -->
<script lang="ts">
	import { Dialog } from 'bits-ui';
	import type { Snippet } from 'svelte';

	interface Props {
		open?: boolean;
		title: string;
		description?: string;
		children: Snippet;
		footer?: Snippet;
	}

	let { open = $bindable(false), title, description, children, footer }: Props = $props();
</script>

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay class="sy-scrim" />
		<Dialog.Content class="sy-dialog">
			<Dialog.Title class="sy-dialog-title">{title}</Dialog.Title>
			{#if description}
				<Dialog.Description class="sy-dialog-description">{description}</Dialog.Description>
			{/if}
			<div class="sy-dialog-body">{@render children()}</div>
			{#if footer}<div class="sy-dialog-footer">{@render footer()}</div>{/if}
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>

<style>
	:global(.sy-scrim) {
		position: fixed;
		inset: 0;
		z-index: 40;
		background: var(--sy-color-scrim);
	}

	:global(.sy-dialog) {
		position: fixed;
		top: 50%;
		left: 50%;
		z-index: 50;
		display: grid;
		gap: var(--sy-space-12);
		width: min(90vw, calc(var(--sy-control-height-md) * 16));
		max-height: 85vh;
		overflow: auto;
		padding: var(--sy-space-20);
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-raised);
		color: var(--sy-color-text-primary);
		box-shadow: var(--sy-shadow-overlay);
		transform: translate(-50%, -50%);
	}

	:global(.sy-dialog-title) {
		margin: 0;
		font-size: var(--sy-font-size-title);
		font-weight: var(--sy-font-weight-semibold);
		line-height: var(--sy-font-line-title);
	}

	:global(.sy-dialog-description) {
		margin: 0;
		color: var(--sy-color-text-secondary);
	}

	:global(.sy-dialog-footer) {
		display: flex;
		justify-content: flex-end;
		gap: var(--sy-space-8);
	}

	/* Phones: a full-screen sheet (untested in M6). */
	@media (width < 768px) {
		:global(.sy-dialog) {
			inset: 0;
			width: 100%;
			max-height: none;
			border-radius: 0;
			transform: none;
		}
	}
</style>
