import type { IconComponent } from './icon';

/** An entry of a DropdownMenu or ContextMenu: an action or a separator. */
export type MenuItem =
	| {
			label: string;
			onSelect: () => void;
			icon?: IconComponent;
			/** Shown as a hint, e.g. "Ctrl+R"; the keymap binds it. */
			shortcut?: string;
			disabled?: boolean;
			danger?: boolean;
	  }
	| { separator: true };
