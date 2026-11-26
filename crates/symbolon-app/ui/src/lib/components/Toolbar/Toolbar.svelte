<!-- Spec: docs/design/components/app-shell.md -->
<script lang="ts">
	import { Search } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import IconButton from '../IconButton/IconButton.svelte';

	interface Props {
		title: string;
		/** Actions of the current view. */
		actions?: Snippet;
		onpalette: () => void;
	}

	let { title, actions, onpalette }: Props = $props();
</script>

<div class="sy-toolbar">
	<h1>{title}</h1>
	<div class="actions">
		{#if actions}{@render actions()}{/if}
		<IconButton label="Command palette (Ctrl+K)" icon={Search} onclick={onpalette} />
	</div>
</div>

<style>
	.sy-toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 100%;
		padding: 0 var(--sy-space-16);
	}

	h1 {
		margin: 0;
		font-size: var(--sy-font-size-title);
		font-weight: var(--sy-font-weight-semibold);
		line-height: var(--sy-font-line-title);
	}

	.actions {
		display: flex;
		align-items: center;
		gap: var(--sy-space-8);
	}
</style>
