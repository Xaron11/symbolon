<!-- Spec: docs/design/components/spinner.md -->
<script lang="ts">
	interface Props {
		label?: string;
		class?: string;
	}

	let { label = 'Loading', class: className }: Props = $props();
</script>

<span class={['sy-spinner', className]} role="status" aria-label={label}></span>

<style>
	.sy-spinner {
		display: inline-block;
		width: var(--sy-icon-size-md);
		height: var(--sy-icon-size-md);
		border: var(--sy-border-width-strong) solid var(--sy-color-accent-default);
		border-right-color: transparent;
		border-radius: var(--sy-radius-full);
		animation: spin calc(var(--sy-motion-duration-enter) * 5) linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
