<!-- Spec: docs/design/components/toast.md -->
<script lang="ts">
	import { X } from '@lucide/svelte';
	import { toasts as appToasts, type Toasts } from '../../feedback/toasts.svelte';
	import IconButton from '../IconButton/IconButton.svelte';

	let { toasts = appToasts }: { toasts?: Toasts } = $props();
	const calm = $derived(toasts.items.filter((t) => t.tone !== 'error'));
	const errors = $derived(toasts.items.filter((t) => t.tone === 'error'));
</script>

{#snippet list(items: typeof toasts.items)}
	{#each items as toast (toast.id)}
		<li class="toast" data-tone={toast.tone} style:--toast-fg="var(--sy-status-{toast.tone}-fg)">
			<span>{toast.message}</span>
			<IconButton
				label="Dismiss"
				icon={X}
				size="sm"
				onclick={() => {
					toasts.dismiss(toast.id);
				}}
			/>
		</li>
	{/each}
{/snippet}

<div class="sy-toasts">
	<ol aria-live="assertive">{@render list(errors)}</ol>
	<ol aria-live="polite">{@render list(calm)}</ol>
</div>

<style>
	.sy-toasts {
		position: fixed;
		right: var(--sy-space-16);
		bottom: calc(var(--sy-statusbar-height) + var(--sy-space-16));
		z-index: 60;
		display: grid;
		gap: var(--sy-space-8);
	}

	ol {
		display: grid;
		gap: var(--sy-space-8);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.toast {
		display: flex;
		align-items: center;
		gap: var(--sy-space-12);
		min-width: calc(var(--sy-control-height-md) * 8);
		padding: var(--sy-space-8) var(--sy-space-8) var(--sy-space-8) var(--sy-space-12);
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-left: var(--sy-border-width-strong) solid var(--toast-fg);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-raised);
		box-shadow: var(--sy-shadow-overlay);
	}

	.toast span {
		flex: 1;
	}
</style>
