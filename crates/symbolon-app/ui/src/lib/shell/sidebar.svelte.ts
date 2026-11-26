import type { KeyValueStore } from '../storage';

const KEY = 'symbolon.sidebar';

/** Whether the sidebar shows labels or icons only; remembered across sessions. */
export interface Sidebar {
	readonly collapsed: boolean;
	toggle(): void;
}

/** Creates the sidebar state from `storage`; storage failures only lose the memory. */
export function createSidebar(storage: KeyValueStore | undefined): Sidebar {
	let collapsed = $state(read());

	function read() {
		try {
			return storage?.getItem(KEY) === 'collapsed';
		} catch {
			return false;
		}
	}

	return {
		get collapsed() {
			return collapsed;
		},
		toggle() {
			collapsed = !collapsed;
			try {
				storage?.setItem(KEY, collapsed ? 'collapsed' : 'expanded');
			} catch {
				// The choice still applies for this session.
			}
		}
	};
}
