<!-- Spec: docs/design/components/empty-state.md -->
<script lang="ts">
	import type { IconComponent } from '../icon';
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		description?: string;
		icon?: IconComponent;
		actions?: Snippet;
		class?: string;
	}

	let { title, description, icon: Icon, actions, class: className }: Props = $props();
</script>

<section class={['sy-empty', className]}>
	{#if Icon}<Icon aria-hidden="true" />{/if}
	<h2>{title}</h2>
	{#if description}<p>{description}</p>{/if}
	{#if actions}<div class="actions">{@render actions()}</div>{/if}
</section>

<style>
	.sy-empty {
		display: grid;
		justify-items: center;
		gap: var(--sy-space-8);
		padding: var(--sy-space-48) var(--sy-space-24);
		color: var(--sy-color-text-secondary);
		text-align: center;
	}

	.sy-empty :global(svg) {
		width: calc(var(--sy-icon-size-md) * 2);
		height: calc(var(--sy-icon-size-md) * 2);
		color: var(--sy-color-text-muted);
	}

	h2 {
		margin: 0;
		color: var(--sy-color-text-primary);
		font-size: var(--sy-font-size-title);
		font-weight: var(--sy-font-weight-semibold);
		line-height: var(--sy-font-line-title);
	}

	p {
		max-width: 40ch;
		margin: 0;
	}

	.actions {
		display: flex;
		gap: var(--sy-space-8);
		margin-top: var(--sy-space-8);
	}
</style>
