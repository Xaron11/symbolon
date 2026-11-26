<!-- Spec: docs/design/components/shortcut-help.md -->
<script lang="ts">
	import Dialog from '../Dialog/Dialog.svelte';
	import Kbd from '../Kbd/Kbd.svelte';
	import { formatKey, KEYMAP, type KeyContext } from '../../keyboard/keymap';

	let { open = $bindable(false) }: { open?: boolean } = $props();
	const sections: readonly { context: KeyContext; title: string }[] = [
		{ context: 'global', title: 'Everywhere' },
		{ context: 'list', title: 'In tables and trees' }
	];
</script>

<Dialog
	bind:open
	title="Keyboard shortcuts"
	description="Vi-style and standard keys work side by side."
>
	{#each sections as section (section.context)}
		<h3>{section.title}</h3>
		<dl>
			{#each KEYMAP.filter((b) => b.context === section.context) as binding (binding.action)}
				<dt>
					{#each binding.keys as key, i (key)}{#if i > 0}<span class="or">or</span>{/if}<Kbd
							keys={formatKey(key)}
						/>{/each}
				</dt>
				<dd>{binding.description}</dd>
			{/each}
		</dl>
	{/each}
</Dialog>

<style>
	h3 {
		margin: var(--sy-space-8) 0 var(--sy-space-4);
		color: var(--sy-color-text-secondary);
		font-size: var(--sy-font-size-small);
		font-weight: var(--sy-font-weight-semibold);
	}

	dl {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: var(--sy-space-4) var(--sy-space-16);
		margin: 0;
	}

	dt {
		display: flex;
		align-items: center;
		gap: var(--sy-space-4);
	}

	dd {
		margin: 0;
	}

	.or {
		color: var(--sy-color-text-muted);
		font-size: var(--sy-font-size-small);
	}
</style>
