import { afterEach, describe, expect, it } from 'vitest';
import { createTheme } from './theme.svelte';

class FakeMedia {
	listeners = new Set<(event: { matches: boolean }) => void>();
	constructor(public matches: boolean) {}
	addEventListener(_: 'change', listener: (event: { matches: boolean }) => void) {
		this.listeners.add(listener);
	}
	removeEventListener(_: 'change', listener: (event: { matches: boolean }) => void) {
		this.listeners.delete(listener);
	}
	change(matches: boolean) {
		this.matches = matches;
		for (const listener of this.listeners) listener({ matches });
	}
}

function storage(initial: Record<string, string> = {}) {
	const values = new Map(Object.entries(initial));
	return {
		values,
		getItem: (key: string) => values.get(key) ?? null,
		setItem: (key: string, value: string) => values.set(key, value)
	};
}

const media = (matches: boolean) => new FakeMedia(matches) as unknown as MediaQueryList;
const root = () => document.documentElement;

afterEach(() => {
	delete root().dataset['theme'];
});

describe('createTheme', () => {
	it('defaults to system and resolves from the media query', () => {
		const theme = createTheme(storage(), media(true));
		expect(theme.preference).toBe('system');
		expect(theme.resolved).toBe('dark');
		expect(root().dataset['theme']).toBeUndefined();
	});

	it('follows the OS while on system', () => {
		const query = new FakeMedia(false);
		const theme = createTheme(storage(), query as unknown as MediaQueryList);
		expect(theme.resolved).toBe('light');
		query.change(true);
		expect(theme.resolved).toBe('dark');
		theme.destroy();
		expect(query.listeners.size).toBe(0);
	});

	it('stores and applies an explicit preference', () => {
		const store = storage();
		const theme = createTheme(store, media(true));
		theme.set('light');
		expect(store.values.get('symbolon.theme')).toBe('light');
		expect(theme.resolved).toBe('light');
		expect(root().dataset['theme']).toBe('light');
		theme.set('system');
		expect(store.values.get('symbolon.theme')).toBe('system');
		expect(root().dataset['theme']).toBeUndefined();
		expect(theme.resolved).toBe('dark');
	});

	it('restores a stored preference', () => {
		const theme = createTheme(storage({ 'symbolon.theme': 'dark' }), media(false));
		expect(theme.preference).toBe('dark');
		expect(theme.resolved).toBe('dark');
		expect(root().dataset['theme']).toBe('dark');
	});

	it('ignores unreadable or invalid storage', () => {
		const broken = {
			getItem: () => {
				throw new Error('denied');
			},
			setItem: () => {
				throw new Error('denied');
			}
		};
		const theme = createTheme(broken, media(false));
		expect(theme.preference).toBe('system');
		expect(() => {
			theme.set('dark');
		}).not.toThrow();
		expect(theme.resolved).toBe('dark');
		expect(createTheme(storage({ 'symbolon.theme': 'blue' }), media(false)).preference).toBe(
			'system'
		);
		expect(createTheme(undefined, media(false)).preference).toBe('system');
	});
});
