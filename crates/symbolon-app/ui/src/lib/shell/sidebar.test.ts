import { describe, expect, it } from 'vitest';
import { createSidebar } from './sidebar.svelte';

const storage = (initial: Record<string, string> = {}) => {
	const values = new Map(Object.entries(initial));
	return {
		values,
		getItem: (k: string) => values.get(k) ?? null,
		setItem: (k: string, v: string) => values.set(k, v)
	};
};

describe('createSidebar', () => {
	it('starts expanded, toggles and remembers', () => {
		const store = storage();
		const sidebar = createSidebar(store);
		expect(sidebar.collapsed).toBe(false);
		sidebar.toggle();
		expect(sidebar.collapsed).toBe(true);
		expect(store.values.get('symbolon.sidebar')).toBe('collapsed');
		expect(createSidebar(store).collapsed).toBe(true);
	});

	it('works without usable storage', () => {
		const broken = {
			getItem: () => {
				throw new Error('denied');
			},
			setItem: () => {
				throw new Error('denied');
			}
		};
		const sidebar = createSidebar(broken);
		expect(sidebar.collapsed).toBe(false);
		expect(() => {
			sidebar.toggle();
		}).not.toThrow();
		expect(sidebar.collapsed).toBe(true);
		expect(createSidebar(undefined).collapsed).toBe(false);
	});
});
