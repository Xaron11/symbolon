<!-- Spec: docs/design/components/app-shell.md -->
<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		sidebar: Snippet;
		toolbar: Snippet;
		status: Snippet;
		/** The current view; the skip link leads here. */
		children: Snippet;
	}

	let { sidebar, toolbar, status, children }: Props = $props();
</script>

<a class="skip" href="#main">Skip to content</a>
<div class="sy-shell">
	<div class="sidebar">{@render sidebar()}</div>
	<header class="toolbar">{@render toolbar()}</header>
	<main id="main" tabindex="-1">{@render children()}</main>
	<footer class="status" aria-label="Status">{@render status()}</footer>
</div>

<style>
	.skip {
		position: absolute;
		top: var(--sy-space-8);
		left: var(--sy-space-8);
		z-index: 100;
		padding: var(--sy-space-4) var(--sy-space-8);
		border-radius: var(--sy-control-radius);
		background: var(--sy-color-accent-default);
		color: var(--sy-color-text-on-accent);
		transform: translateY(-200%);
	}

	.skip:focus {
		transform: none;
	}

	.sy-shell {
		display: grid;
		grid-template:
			'sidebar toolbar' var(--sy-toolbar-height)
			'sidebar main' 1fr
			'status status' var(--sy-statusbar-height)
			/ auto 1fr;
		height: 100vh;
	}

	.sidebar {
		grid-area: sidebar;
		min-height: 0;
	}

	.toolbar {
		grid-area: toolbar;
		border-bottom: var(--sy-border-width-default) solid var(--sy-color-border-default);
		background: var(--sy-color-surface-base);
	}

	main {
		/* Absolutely positioned content (visually hidden text) must scroll with the view, not the page. */
		position: relative;
		grid-area: main;
		min-width: 0;
		min-height: 0;
		overflow: auto;
		padding: var(--sy-space-16);
		outline: none;
	}

	.status {
		grid-area: status;
		border-top: var(--sy-border-width-default) solid var(--sy-color-border-default);
		background: var(--sy-color-surface-sunken);
	}

	/* Phones: the sidebar becomes a bottom bar (untested in M6). */
	@media (width < 768px) {
		.sy-shell {
			grid-template:
				'toolbar' var(--sy-toolbar-height)
				'main' 1fr
				'sidebar' auto
				'status' var(--sy-statusbar-height)
				/ 1fr;
		}
	}
</style>
