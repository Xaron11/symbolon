// The one table of keyboard shortcuts (ADR-0022): the dispatcher, the shortcut help, the command
// palette and the book's keyboard page all read it. No imports, so build scripts can load it.

/** Where a binding applies: everywhere, or while a table or tree has focus. */
export type KeyContext = 'global' | 'list';

/** Every action a key can trigger. */
export const ACTIONS = [
	'palette.open',
	'help.open',
	'escape',
	'nav.locations',
	'nav.scans',
	'nav.compare',
	'sidebar.toggle',
	'list.down',
	'list.up',
	'list.collapse',
	'list.expand',
	'list.top',
	'list.bottom',
	'list.halfPageDown',
	'list.halfPageUp',
	'list.pageDown',
	'list.pageUp',
	'list.open',
	'list.toggleSelect',
	'list.filter',
	'list.nextMatch',
	'list.prevMatch'
] as const;

/** An action from {@link ACTIONS}. */
export type Action = (typeof ACTIONS)[number];

/**
 * A binding of keys to an action. A key is a chord of lowercase names joined by `+`, with
 * modifiers first (`ctrl+k`, `shift+g`, `arrowdown`, `space`, `?`), or a sequence of chords
 * separated by spaces (`g l`). `ctrl` also matches the Command key.
 */
export interface Binding {
	readonly action: Action;
	readonly context: KeyContext;
	readonly keys: readonly string[];
	readonly description: string;
}

/** Every binding. Vi-style keys are always on, beside the standard ones. */
export const KEYMAP: readonly Binding[] = [
	{
		action: 'palette.open',
		context: 'global',
		keys: ['ctrl+k'],
		description: 'Open the command palette'
	},
	{ action: 'help.open', context: 'global', keys: ['?'], description: 'Show keyboard shortcuts' },
	{ action: 'escape', context: 'global', keys: ['escape'], description: 'Close or cancel' },
	{
		action: 'nav.locations',
		context: 'global',
		keys: ['ctrl+1', 'g l'],
		description: 'Go to Locations'
	},
	{ action: 'nav.scans', context: 'global', keys: ['ctrl+2', 'g s'], description: 'Go to Scans' },
	{
		action: 'nav.compare',
		context: 'global',
		keys: ['ctrl+3', 'g c'],
		description: 'Go to Compare'
	},
	{
		action: 'sidebar.toggle',
		context: 'global',
		keys: ['ctrl+b'],
		description: 'Collapse or expand the sidebar'
	},
	{ action: 'list.down', context: 'list', keys: ['j', 'arrowdown'], description: 'Next row' },
	{ action: 'list.up', context: 'list', keys: ['k', 'arrowup'], description: 'Previous row' },
	{
		action: 'list.collapse',
		context: 'list',
		keys: ['h', 'arrowleft'],
		description: 'Collapse folder, or previous column'
	},
	{
		action: 'list.expand',
		context: 'list',
		keys: ['l', 'arrowright'],
		description: 'Expand folder, or next column'
	},
	{ action: 'list.top', context: 'list', keys: ['g g', 'home'], description: 'First row' },
	{ action: 'list.bottom', context: 'list', keys: ['shift+g', 'end'], description: 'Last row' },
	{
		action: 'list.halfPageDown',
		context: 'list',
		keys: ['ctrl+d'],
		description: 'Half a page down'
	},
	{ action: 'list.halfPageUp', context: 'list', keys: ['ctrl+u'], description: 'Half a page up' },
	{ action: 'list.pageDown', context: 'list', keys: ['pagedown'], description: 'Page down' },
	{ action: 'list.pageUp', context: 'list', keys: ['pageup'], description: 'Page up' },
	{ action: 'list.open', context: 'list', keys: ['enter', 'o'], description: 'Open the row' },
	{
		action: 'list.toggleSelect',
		context: 'list',
		keys: ['space', 'x'],
		description: 'Select or deselect the row'
	},
	{ action: 'list.filter', context: 'list', keys: ['/'], description: 'Filter the list' },
	{ action: 'list.nextMatch', context: 'list', keys: ['n'], description: 'Next match' },
	{ action: 'list.prevMatch', context: 'list', keys: ['shift+n'], description: 'Previous match' }
];

const NAMES: Readonly<Record<string, string>> = {
	ctrl: 'Ctrl',
	alt: 'Alt',
	shift: 'Shift',
	arrowdown: '↓',
	arrowup: '↑',
	arrowleft: '←',
	arrowright: '→',
	escape: 'Esc',
	enter: 'Enter',
	space: 'Space',
	home: 'Home',
	end: 'End',
	pagedown: 'PageDown',
	pageup: 'PageUp'
};

/** A key as people read it: `ctrl+k` → `Ctrl+K`, `arrowdown` → `↓`; sequences keep their case. */
export function formatKey(key: string): string {
	if (key.includes(' ')) return key;
	return key
		.split('+')
		.map(
			(part) => NAMES[part] ?? (part.length === 1 && key.includes('+') ? part.toUpperCase() : part)
		)
		.join('+');
}
