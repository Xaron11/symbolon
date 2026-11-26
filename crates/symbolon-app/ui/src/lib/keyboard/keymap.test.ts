import { describe, expect, it } from 'vitest';
import { ACTIONS, formatKey, KEYMAP } from './keymap';

describe('KEYMAP', () => {
	it('binds every action', () => {
		expect([...new Set(KEYMAP.map((b) => b.action))].sort()).toEqual([...ACTIONS].sort());
	});

	it('never binds one key twice where contexts overlap', () => {
		const seen = new Map<string, string>();
		for (const binding of KEYMAP) {
			for (const key of binding.keys) {
				// Global bindings are active in every context, so they clash with list ones too.
				const other = seen.get(key);
				expect(other, `${key} bound to ${binding.action} and ${other ?? ''}`).toBeUndefined();
				seen.set(key, binding.action);
			}
		}
	});

	it('formats keys for display', () => {
		expect(formatKey('ctrl+k')).toBe('Ctrl+K');
		expect(formatKey('shift+g')).toBe('Shift+G');
		expect(formatKey('arrowdown')).toBe('↓');
		expect(formatKey('g l')).toBe('g l');
		expect(formatKey('space')).toBe('Space');
		expect(formatKey('?')).toBe('?');
	});
});
