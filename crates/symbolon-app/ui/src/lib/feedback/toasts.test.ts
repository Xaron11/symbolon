import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createToasts } from './toasts.svelte';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('toasts', () => {
	it('dismisses information after the timeout', () => {
		const toasts = createToasts();
		toasts.show('Scan finished');
		expect(toasts.items.map((t) => t.message)).toEqual(['Scan finished']);
		vi.advanceTimersByTime(5000);
		expect(toasts.items).toEqual([]);
	});

	it('keeps errors until dismissed', () => {
		const toasts = createToasts();
		const id = toasts.show('Scan failed', 'error');
		vi.advanceTimersByTime(60_000);
		expect(toasts.items.map((t) => t.tone)).toEqual(['error']);
		toasts.dismiss(id);
		expect(toasts.items).toEqual([]);
	});

	it('honours a custom timeout and keeps order', () => {
		const toasts = createToasts();
		toasts.show('one', 'info', 1000);
		toasts.show('two', 'success');
		vi.advanceTimersByTime(1000);
		expect(toasts.items.map((t) => t.message)).toEqual(['two']);
	});
});
