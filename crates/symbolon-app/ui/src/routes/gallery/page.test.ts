import { describe, expect, it, vi } from 'vitest';

vi.mock('$app/env', () => ({ dev: false }));

describe('gallery route', () => {
	it('does not exist in release builds', async () => {
		const { load } = await import('./+page');
		expect(() => {
			load();
		}).toThrow(expect.objectContaining({ status: 404 }));
	});
});
