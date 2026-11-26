<!-- Spec: docs/design/components/alert.md -->
<script lang="ts">
	import { CircleAlert, CircleCheck, Info, TriangleAlert } from '@lucide/svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		tone: 'error' | 'warning' | 'info' | 'success';
		title: string;
		children?: Snippet;
		class?: string;
	}

	let { tone, title, children, class: className }: Props = $props();
	const icons = { error: CircleAlert, warning: TriangleAlert, info: Info, success: CircleCheck };
	const Icon = $derived(icons[tone]);
</script>

<div
	class={['sy-alert', className]}
	data-tone={tone}
	role={tone === 'error' ? 'alert' : 'status'}
	style:--alert-fg="var(--sy-status-{tone}-fg)"
	style:--alert-bg="var(--sy-status-{tone}-bg)"
	style:--alert-border="var(--sy-status-{tone}-border)"
>
	<Icon aria-hidden="true" />
	<div>
		<strong>{title}</strong>
		{#if children}<div class="body">{@render children()}</div>{/if}
	</div>
</div>

<style>
	.sy-alert {
		display: flex;
		gap: var(--sy-space-8);
		padding: var(--sy-space-8) var(--sy-space-12);
		border: var(--sy-border-width-default) solid var(--alert-border);
		border-radius: var(--sy-panel-radius);
		background: var(--alert-bg);
		color: var(--sy-color-text-primary);
	}

	.sy-alert :global(svg) {
		flex: none;
		width: var(--sy-icon-size-md);
		height: var(--sy-icon-size-md);
		margin-top: var(--sy-space-2);
		color: var(--alert-fg);
	}

	strong {
		color: var(--alert-fg);
		font-weight: var(--sy-font-weight-semibold);
	}

	.body {
		color: var(--sy-color-text-secondary);
	}
</style>
