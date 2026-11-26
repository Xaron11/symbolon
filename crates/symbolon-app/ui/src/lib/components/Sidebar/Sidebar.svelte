<!-- Spec: docs/design/components/app-shell.md -->
<script lang="ts">
	import { PanelLeftClose, PanelLeftOpen } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import type { IconComponent } from '../icon';
	import IconButton from '../IconButton/IconButton.svelte';
	import { formatKey } from '../../keyboard/keymap';

	interface Item {
		href: string;
		label: string;
		icon: IconComponent;
		shortcut: string;
	}

	interface Props {
		items: readonly Item[];
		/** The href of the current section. */
		current: string | undefined;
		collapsed: boolean;
		ontoggle: () => void;
		/** Settings at the bottom (shown while expanded). */
		footer?: Snippet;
	}

	let { items, current, collapsed, ontoggle, footer }: Props = $props();
</script>

<nav class="sy-sidebar" aria-label="Sections" data-collapsed={collapsed || undefined}>
	<ul>
		{#each items as item (item.href)}
			{@const Icon = item.icon}
			<li>
				<a
					href={item.href}
					aria-current={item.href === current ? 'page' : undefined}
					title="{item.label} ({formatKey(item.shortcut)})"
				>
					<Icon aria-hidden="true" />
					<span class:sy-visually-hidden={collapsed}>{item.label}</span>
				</a>
			</li>
		{/each}
	</ul>
	<div class="bottom">
		{#if footer && !collapsed}{@render footer()}{/if}
		<IconButton
			label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
			icon={collapsed ? PanelLeftOpen : PanelLeftClose}
			onclick={ontoggle}
		/>
	</div>
</nav>

<style>
	.sy-sidebar {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		width: var(--sy-sidebar-width-expanded);
		height: 100%;
		padding: var(--sy-space-8);
		border-right: var(--sy-border-width-default) solid var(--sy-color-border-default);
		background: var(--sy-color-surface-base);
	}

	[data-collapsed] {
		width: var(--sy-sidebar-width-collapsed);
		padding: var(--sy-space-8) var(--sy-space-4);
	}

	ul {
		display: grid;
		gap: var(--sy-space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	a {
		display: flex;
		align-items: center;
		gap: var(--sy-space-8);
		height: var(--sy-control-height-md);
		padding: 0 var(--sy-space-8);
		border-left: var(--sy-border-width-strong) solid transparent;
		border-radius: var(--sy-control-radius);
		color: var(--sy-color-text-secondary);
		text-decoration: none;
	}

	a :global(svg) {
		flex: none;
		width: var(--sy-icon-size-md);
		height: var(--sy-icon-size-md);
	}

	a:hover {
		background: var(--sy-color-surface-hover);
		color: var(--sy-color-text-primary);
	}

	a[aria-current='page'] {
		border-left-color: var(--sy-color-accent-default);
		background: var(--sy-color-surface-selected);
		color: var(--sy-color-text-primary);
		font-weight: var(--sy-font-weight-medium);
	}

	.bottom {
		display: grid;
		gap: var(--sy-space-8);
		justify-items: start;
	}

	@media (pointer: coarse) {
		a {
			min-height: var(--sy-control-height-touch);
		}
	}

	/* Phones: a bottom bar of sections (untested in M6). */
	@media (width < 768px) {
		.sy-sidebar {
			flex-direction: row;
			width: 100%;
			height: auto;
			border-top: var(--sy-border-width-default) solid var(--sy-color-border-default);
			border-right: 0;
		}

		ul {
			grid-auto-flow: column;
			flex: 1;
		}

		.bottom {
			display: none;
		}
	}
</style>
