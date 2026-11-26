import { getContext, setContext } from 'svelte';
import type { Action } from './keymap';

const KEY = Symbol('sy-global-actions');

/**
 * Makes `run` handle app-wide actions (navigation, palette, help) for lists inside this
 * component. A focused list sees every key first, so it matches app-wide bindings too and
 * forwards them here; a separate window dispatcher would lose sequences such as `g l` to the
 * list's `g g`.
 */
export function provideGlobalActions(run: (action: Action) => void): void {
	setContext(KEY, run);
}

/** The handler for app-wide actions set by an enclosing shell, or undefined outside one. */
export function globalActions(): ((action: Action) => void) | undefined {
	return getContext<((action: Action) => void) | undefined>(KEY);
}
