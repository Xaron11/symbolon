/** The parts of `Storage` the app uses; tests pass fakes. */
export type KeyValueStore = Pick<Storage, 'getItem' | 'setItem'>;

/** `localStorage`, or undefined where reading it throws (blocked or sandboxed storage). */
export function browserStorage(): KeyValueStore | undefined {
	try {
		return globalThis.localStorage;
	} catch {
		return undefined;
	}
}
