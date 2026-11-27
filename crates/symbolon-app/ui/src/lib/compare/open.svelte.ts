import { getContext, setContext } from 'svelte';
import type { Backend, CompareRequest } from '../api/index.ts';
import { createCompareSession, type CompareSession } from './session.svelte';

/** The comparison that stays open while people visit other screens. */
export interface OpenComparison {
	/** The open comparison's page, such as `/compare/view?sides=…`; undefined when none is open. */
	readonly href: string | undefined;
	/** The open session when `query` names it, otherwise a newly opened one that replaces it. */
	session(query: string, request: CompareRequest): CompareSession;
	/** Closes the open comparison. */
	close(): void;
}

/** Keeps at most one comparison open on `backend`. */
export function createOpenComparison(backend: Backend): OpenComparison {
	let current = $state.raw<{ query: string; session: CompareSession } | undefined>(undefined);

	return {
		get href() {
			return current ? `/compare/view?${current.query}` : undefined;
		},
		session(query, request) {
			// One that failed to open is tried again rather than shown failing forever.
			if (current?.query === query && current.session.error === undefined) {
				return current.session;
			}
			current?.session.close();
			const session = createCompareSession(backend, request);
			void session.open();
			current = { query, session };
			return session;
		},
		close() {
			current?.session.close();
			current = undefined;
		}
	};
}

const KEY = Symbol('open-comparison');

/** Makes `open` the open comparison of this component's descendants. */
export function provideOpenComparison(open: OpenComparison): void {
	setContext(KEY, open);
}

/** The open comparison provided by an ancestor; throws when there is none. */
export function openComparison(): OpenComparison {
	const found = getContext<OpenComparison | undefined>(KEY);
	if (!found)
		throw new Error('no open comparison provided; call provideOpenComparison in a parent');
	return found;
}
