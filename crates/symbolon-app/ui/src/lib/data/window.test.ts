import { describe, expect, it } from 'vitest';
import type { DataSource, SortSpec } from './source';
import { memorySource } from './memory';
import { createRowWindow } from './window.svelte';

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const numbers = (n: number) =>
	Array.from({ length: n }, (_, i) => ({ id: i, name: `file${String(i)}` }));

describe('createRowWindow', () => {
	it('renders only the visible window', async () => {
		const source = memorySource(numbers(100_000));
		const win = createRowWindow(source, { rowHeight: 28, overscan: 5 });
		await settle();
		win.setViewport(0, 280);
		await settle();
		expect(win.count).toBe(100_000);
		expect(win.start).toBe(0);
		expect(win.rows.length).toBeLessThanOrEqual(10 + 2 * 5);
		expect(win.rows[0]).toEqual({ id: 0, name: 'file0' });
	});

	it('coalesces fast scrolling into requests for the final window', async () => {
		const source = memorySource(numbers(200_000));
		const win = createRowWindow(source, { rowHeight: 28, overscan: 5, pageSize: 200 });
		await settle();
		source.calls.length = 0;
		for (let i = 0; i <= 50; i++) win.setViewport((i / 50) * (200_000 * 28 - 280), 280);
		await settle();
		expect(source.calls.length).toBeLessThanOrEqual(2);
		expect(win.rows.at(-1)).toEqual({ id: 199_999, name: 'file199999' });
	});

	it('drops responses that arrive after the sort changed', async () => {
		const pending: ((rows: { id: number }[]) => void)[] = [];
		const source: DataSource<{ id: number }> = {
			count: () => Promise.resolve(3),
			rows: (offset, limit, sort?: SortSpec) =>
				new Promise((resolve) => {
					pending.push(() => {
						resolve(
							Array.from({ length: Math.min(limit, 3 - offset) }, (_, i) => ({
								id: sort?.direction === 'desc' ? 2 - i : i
							}))
						);
					});
				})
		};
		const win = createRowWindow(source, { rowHeight: 28 });
		await settle();
		win.setViewport(0, 280);
		await settle();
		win.setSort({ column: 'id', direction: 'desc' });
		await settle();
		pending[1]?.([]);
		await settle();
		pending[0]?.([]);
		await settle();
		expect(win.rows.map((r) => r?.id)).toEqual([2, 1, 0]);
	});

	it('surfaces errors and retries', async () => {
		let fail = true;
		const source: DataSource<{ id: number }> = {
			count: () => Promise.resolve(1),
			rows: () => (fail ? Promise.reject(new Error('gone')) : Promise.resolve([{ id: 0 }]))
		};
		const win = createRowWindow(source, { rowHeight: 28 });
		await settle();
		win.setViewport(0, 280);
		await settle();
		expect(win.error?.message).toBe('gone');
		fail = false;
		win.retry();
		await settle();
		await settle();
		expect(win.error).toBeUndefined();
		expect(win.rows).toEqual([{ id: 0 }]);
	});
});
