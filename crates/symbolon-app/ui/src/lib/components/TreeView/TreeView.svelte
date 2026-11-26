<!-- Spec: docs/design/components/tree-view.md -->
<script lang="ts" generics="Node">
	import { ChevronRight } from '@lucide/svelte';
	import { component } from '../../design/generated/tokens';
	import type { TreeSource } from '../../data/source';
	import { createTreeState } from '../../data/tree.svelte';
	import { revealScroll } from '../../data/window.svelte';
	import { createDispatcher } from '../../keyboard/dispatcher';
	import { KEYMAP, type Action } from '../../keyboard/keymap';
	import type { IconComponent } from '../icon';

	interface Props {
		/** Read once; give the tree a new `{#key}` to show another source. */
		source: TreeSource<Node>;
		label: (node: Node) => string;
		icon?: (node: Node) => IconComponent | undefined;
		onopen?: (node: Node) => void;
		/** Accessible name of the tree. */
		name: string;
	}

	let { source, label, icon, onopen, name }: Props = $props();

	const rowHeight = parseFloat(component['table.row.height']);
	const overscan = 10;
	// svelte-ignore state_referenced_locally
	const tree = createTreeState(source);
	const id = $props.id();
	let active = $state(0);
	let view = $state<HTMLDivElement>();
	let scroller = $state<HTMLDivElement>();
	let scrollTop = $state(0);
	let height = $state(0);

	const items = $derived(tree.visible);
	const first = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - overscan));
	const last = $derived(
		Math.min(items.length - 1, Math.ceil((scrollTop + height) / rowHeight) + overscan)
	);
	const shown = $derived(items.slice(first, last + 1));
	const visibleRows = $derived(Math.max(1, Math.floor(height / rowHeight)));

	function moveTo(index: number) {
		if (items.length === 0) return;
		active = Math.min(items.length - 1, Math.max(0, index));
		if (scroller) {
			scroller.scrollTop = revealScroll({
				index: active,
				scrollTop: scroller.scrollTop,
				height,
				rowHeight,
				headerHeight: 0
			});
			scrollTop = scroller.scrollTop;
		}
	}

	function onAction(action: Action) {
		const item = items[active];
		switch (action) {
			case 'list.down':
				moveTo(active + 1);
				break;
			case 'list.up':
				moveTo(active - 1);
				break;
			case 'list.top':
				moveTo(0);
				break;
			case 'list.bottom':
				moveTo(items.length - 1);
				break;
			case 'list.halfPageDown':
				moveTo(active + Math.ceil(visibleRows / 2));
				break;
			case 'list.halfPageUp':
				moveTo(active - Math.ceil(visibleRows / 2));
				break;
			case 'list.pageDown':
				moveTo(active + visibleRows);
				break;
			case 'list.pageUp':
				moveTo(active - visibleRows);
				break;
			case 'list.expand':
				// Expand a closed folder; in an open one, step to its first child.
				if (item?.hasChildren && !item.expanded) void tree.expand(item.key);
				else if (item?.expanded) moveTo(active + 1);
				break;
			case 'list.collapse':
				// Collapse an open folder; otherwise step to the parent.
				if (item?.expanded) tree.collapse(item.key);
				else if (item?.parent !== undefined) {
					const parent = item.parent;
					moveTo(items.findIndex((v) => v.key === parent));
				}
				break;
			case 'list.open':
				if (item) onopen?.(item.node);
				break;
			case 'list.toggleSelect':
			case 'list.sort':
			case 'list.filter':
			case 'list.nextMatch':
			case 'list.prevMatch':
				// Selection and filtering belong to the view that owns the tree.
				break;
			case 'escape':
			case 'help.open':
			case 'palette.open':
			case 'nav.locations':
			case 'nav.scans':
			case 'nav.compare':
			case 'sidebar.toggle':
				// Global actions; only list bindings are active here.
				break;
		}
	}

	const dispatch = createDispatcher(KEYMAP, () => ['list'], onAction);
</script>

<div
	bind:this={view}
	class="sy-tree"
	role="tree"
	aria-label={name}
	aria-activedescendant={active >= first && active <= last && items[active]
		? `${id}-n${String(active)}`
		: undefined}
	tabindex="0"
	onkeydown={(event) => {
		dispatch(event);
	}}
>
	<div
		class="body"
		bind:this={scroller}
		bind:clientHeight={height}
		onscroll={() => {
			requestAnimationFrame(() => {
				scrollTop = scroller?.scrollTop ?? 0;
			});
		}}
	>
		<div class="spacer" role="presentation" style:height="{String(items.length * rowHeight)}px">
			{#each shown as item, i (item.key)}
				{@const index = first + i}
				{@const Icon = icon?.(item.node)}
				<!-- Items are reached through the tree (aria-activedescendant), not focused themselves;
				     clicking one makes it active and keeps focus on the tree. -->
				<!-- svelte-ignore a11y_click_events_have_key_events, a11y_interactive_supports_focus -->
				<div
					id="{id}-n{index}"
					class="item"
					class:active={index === active}
					role="treeitem"
					aria-level={item.level}
					aria-setsize={item.setSize}
					aria-posinset={item.posInSet}
					aria-expanded={item.hasChildren ? item.expanded : undefined}
					aria-selected={index === active}
					style:transform="translateY({String(index * rowHeight)}px)"
					style:--depth={item.level - 1}
					onclick={() => {
						active = index;
						view?.focus();
					}}
				>
					<span
						class="chevron"
						class:open={item.expanded}
						aria-hidden="true"
						onclick={() => void tree.toggle(item.key)}
					>
						{#if item.hasChildren}<ChevronRight />{/if}
					</span>
					{#if Icon}<Icon aria-hidden="true" />{/if}
					<span class="label" title={label(item.node)}>{label(item.node)}</span>
				</div>
			{/each}
		</div>
	</div>
	{#if tree.error}
		<p class="error" role="alert">Could not read folders: {tree.error.message}</p>
	{/if}
</div>

<style>
	.sy-tree {
		height: 100%;
		min-height: 0;
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-base);
		outline-offset: calc(var(--sy-focus-ring-offset) * -1);
	}

	.body {
		height: 100%;
		overflow: auto;
	}

	.spacer {
		position: relative;
	}

	.item {
		position: absolute;
		top: 0;
		right: 0;
		left: 0;
		display: flex;
		align-items: center;
		gap: var(--sy-space-4);
		height: var(--sy-table-row-height);
		padding-right: var(--sy-space-8);
		padding-left: calc(var(--sy-tree-indent) * var(--depth) + var(--sy-space-4));
		border-left: var(--sy-border-width-strong) solid transparent;
		white-space: nowrap;
		cursor: default;
	}

	.item:hover {
		background: var(--sy-color-surface-hover);
	}

	.sy-tree:focus-visible .item.active {
		border-left-color: var(--sy-color-accent-default);
		background: var(--sy-color-surface-selected);
	}

	.item :global(svg) {
		flex: none;
		width: var(--sy-icon-size-sm);
		height: var(--sy-icon-size-sm);
		color: var(--sy-color-text-secondary);
	}

	.chevron {
		display: inline-grid;
		place-items: center;
		width: var(--sy-icon-size-md);
		height: var(--sy-icon-size-md);
		transition: transform var(--sy-motion-duration-state) var(--sy-motion-easing-enter);
	}

	.chevron.open {
		transform: rotate(90deg);
	}

	.label {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.error {
		margin: 0;
		padding: var(--sy-space-8);
		color: var(--sy-status-error-fg);
	}
</style>
