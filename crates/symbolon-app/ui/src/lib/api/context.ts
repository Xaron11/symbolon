import { getContext, setContext } from 'svelte';
import type { Backend } from './backend';

const KEY = Symbol('sy-backend');

/** Makes `backend` available to the components below the caller. */
export function provideBackend(backend: Backend): void {
	setContext(KEY, backend);
}

/** The backend provided by an ancestor; throws when there is none. */
export function backend(): Backend {
	const found = getContext<Backend | undefined>(KEY);
	if (!found) throw new Error('no backend provided; call provideBackend in a parent');
	return found;
}
