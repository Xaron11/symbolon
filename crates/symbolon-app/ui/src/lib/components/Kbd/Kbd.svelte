<!-- Spec: docs/design/components/kbd.md -->
<script lang="ts">
	interface Props {
		/** A chord joined by "+" ("Ctrl+K") or a sequence separated by spaces ("g l"). */
		keys: string;
		class?: string;
	}

	let { keys, class: className }: Props = $props();
	const parts = $derived(keys.split(/[+ ]/).filter((k) => k !== ''));
</script>

<kbd class={['sy-kbd', className]}>
	{#each parts as part, i (i)}<kbd>{part}</kbd>{/each}
</kbd>

<style>
	.sy-kbd {
		display: inline-flex;
		gap: var(--sy-space-2);
		font: inherit;
	}

	kbd kbd {
		min-width: var(--sy-icon-size-md);
		padding: 0 var(--sy-space-4);
		border: var(--sy-border-width-default) solid var(--sy-color-border-strong);
		border-radius: var(--sy-radius-xs);
		background: var(--sy-color-surface-sunken);
		color: var(--sy-color-text-secondary);
		font-family: var(--sy-font-family-mono);
		font-size: var(--sy-font-size-small);
		line-height: var(--sy-font-line-small);
		text-align: center;
	}
</style>
