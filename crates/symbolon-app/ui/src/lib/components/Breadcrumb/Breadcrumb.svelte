<!-- Spec: docs/design/components/breadcrumb.md -->
<script lang="ts">
	interface Props {
		/** Pages above this one, outermost first. */
		trail: readonly { label: string; href: string }[];
		/** This page. */
		current: string;
	}

	let { trail, current }: Props = $props();
</script>

<nav class="sy-breadcrumb" aria-label="Breadcrumb">
	{#each trail as step (step.href)}
		<a href={step.href}>{step.label}</a>
		<span aria-hidden="true">›</span>
	{/each}
	<span aria-current="page">{current}</span>
</nav>

<style>
	.sy-breadcrumb {
		display: flex;
		flex-wrap: wrap;
		gap: var(--sy-space-8);
		color: var(--sy-color-text-muted);
		font-size: var(--sy-font-size-small);
	}

	a {
		color: var(--sy-color-text-secondary);
		text-decoration: none;
	}

	a:hover {
		color: var(--sy-color-text-primary);
		text-decoration: underline;
	}
</style>
