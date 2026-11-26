<script lang="ts">
	import type { Snippet } from 'svelte';
	import AppShell from '../components/AppShell/AppShell.svelte';
	import CommandPalette, {
		type PaletteCommand
	} from '../components/CommandPalette/CommandPalette.svelte';
	import ShortcutHelp from '../components/ShortcutHelp/ShortcutHelp.svelte';
	import Sidebar from '../components/Sidebar/Sidebar.svelte';
	import StatusBar from '../components/StatusBar/StatusBar.svelte';
	import ThemeSwitch from '../components/ThemeSwitch/ThemeSwitch.svelte';
	import Toast from '../components/Toast/Toast.svelte';
	import Toolbar from '../components/Toolbar/Toolbar.svelte';
	import { theme } from '../design/theme.svelte';
	import { createDispatcher } from '../keyboard/dispatcher';
	import { provideGlobalActions } from '../keyboard/global';
	import { KEYMAP, type Action } from '../keyboard/keymap';
	import ScanStatus from '../scans/ScanStatus.svelte';
	import type { ScanState } from '../scans/scans.svelte';
	import { browserStorage } from '../storage';
	import { SECTIONS, sectionOf } from './sections';
	import { createSidebar } from './sidebar.svelte';

	interface Props {
		pathname: string;
		/** Opens a route; the app passes SvelteKit's `goto`. */
		navigate: (href: string) => void;
		/** Running scans, for the status bar; none when left out. */
		scans?: ScanState;
		children: Snippet;
	}

	let { pathname, navigate, scans, children }: Props = $props();
	const sidebarState = createSidebar(browserStorage());
	let paletteOpen = $state(false);
	let helpOpen = $state(false);
	const section = $derived(sectionOf(pathname));

	const commands: readonly PaletteCommand[] = [
		...SECTIONS.map((s) => ({
			id: s.action,
			label: `Go to ${s.label}`,
			group: 'Navigate',
			shortcut: s.shortcut,
			run: () => {
				navigate(s.href);
			}
		})),
		{
			id: 'sidebar',
			label: 'Toggle sidebar',
			group: 'View',
			shortcut: 'ctrl+b',
			run: () => {
				sidebarState.toggle();
			}
		},
		...(['system', 'dark', 'light'] as const).map((preference) => ({
			id: `theme.${preference}`,
			label: `Theme: ${preference[0]?.toUpperCase() ?? ''}${preference.slice(1)}`,
			group: 'Appearance',
			run: () => {
				theme?.set(preference);
			}
		})),
		{
			id: 'help',
			label: 'Keyboard shortcuts',
			group: 'Help',
			shortcut: '?',
			run: () => (helpOpen = true)
		}
	];

	function onAction(action: Action) {
		const target = SECTIONS.find((s) => s.action === action);
		if (target) {
			navigate(target.href);
			return;
		}
		if (action === 'palette.open') paletteOpen = true;
		else if (action === 'help.open') helpOpen = true;
		else if (action === 'sidebar.toggle') sidebarState.toggle();
	}

	provideGlobalActions(onAction);
	const dispatch = createDispatcher(KEYMAP, () => ['global'], onAction);
</script>

<svelte:window
	onkeydown={(event: KeyboardEvent) => {
		dispatch(event);
	}}
/>

<AppShell>
	{#snippet sidebar()}
		<Sidebar
			items={SECTIONS}
			current={section?.href}
			collapsed={sidebarState.collapsed}
			ontoggle={() => {
				sidebarState.toggle();
			}}
		>
			{#snippet footer()}<ThemeSwitch />{/snippet}
		</Sidebar>
	{/snippet}
	{#snippet toolbar()}
		<Toolbar title={section?.label ?? 'Symbolon'} onpalette={() => (paletteOpen = true)} />
	{/snippet}
	{#snippet status()}
		<StatusBar><ScanStatus running={scans?.running ?? []} /></StatusBar>
	{/snippet}
	{@render children()}
</AppShell>

<CommandPalette bind:open={paletteOpen} {commands} />
<ShortcutHelp bind:open={helpOpen} />
<Toast />
