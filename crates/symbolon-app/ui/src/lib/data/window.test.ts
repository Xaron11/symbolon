import { afterEach, describe, expect, it, vi } from 'vitest';
import type { DataSource, SortSpec } from './source';
import { memorySource } from './memory';
import { createRowWindow, revealScroll } from './window.svelte';

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const numbers = (n: number) =>
	Array.from({ length: n }, (_, i) => ({ id: i, name: `file${String(i)}` }));

describe('createRowWindow', () => {
	it('renders only the visible window', async () => {
		const source = memorySource(numbers(100_000));
		const win = createRowWindow(source, { rowHeight: 28, overscan: 5, settleMs: 0 });
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
		const win = createRowWindow(source, { rowHeight: 28, overscan: 5, pageSize: 200, settleMs: 0 });
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
		const win = createRowWindow(source, { rowHeight: 28, settleMs: 0 });
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
		const win = createRowWindow(source, { rowHeight: 28, settleMs: 0 });
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

	describe('while dragging the scrollbar', () => {
		afterEach(() => vi.useRealTimers());

		it('waits for scrolling to settle before reading', async () => {
			vi.useFakeTimers();
			const source = memorySource(numbers(200_000));
			const win = createRowWindow(source, { rowHeight: 28, overscan: 5, pageSize: 200 });
			await vi.runAllTimersAsync();
			source.calls.length = 0;
			for (let frame = 0; frame <= 60; frame++) {
				win.setViewport((frame / 60) * (200_000 * 28 - 280), 280);
				await vi.advanceTimersByTimeAsync(16);
			}
			await vi.runAllTimersAsync();
			expect(source.calls.length).toBeLessThanOrEqual(2);
			expect(win.rows.at(-1)).toEqual({ id: 199_999, name: 'file199999' });
		});

		it('ignores answers for pages scrolled away from, keeping the visible rows', async () => {
			const resolvers = new Map<number, () => void>();
			const all = numbers(100_000);
			const source: DataSource<{ id: number; name: string }> = {
				count: () => Promise.resolve(all.length),
				rows: (offset, limit) =>
					new Promise((resolve) => {
						resolvers.set(offset, () => {
							resolve(all.slice(offset, offset + limit));
						});
					})
			};
			const win = createRowWindow(source, {
				rowHeight: 28,
				overscan: 5,
				pageSize: 200,
				maxPages: 2,
				settleMs: 0
			});
			await settle();
			for (const top of [0, 30_000 * 28, 60_000 * 28, 99_990 * 28]) {
				win.setViewport(top, 280);
				await settle();
			}
			resolvers.get(99_800)?.();
			await settle();
			for (const [offset, resolve] of resolvers) if (offset !== 99_800) resolve();
			await settle();
			expect(win.rows.at(-1)).toEqual({ id: 99_999, name: 'file99999' });
		});
	});
});

describe('revealScroll', () => {
	it('keeps a row below a sticky header in view', () => {
		// Viewport 100px with a 32px header: rows of 28px; row 3 spans 84–112 in the body.
		expect(
			revealScroll({ index: 3, scrollTop: 0, height: 100, rowHeight: 28, headerHeight: 32 })
		).toBe(44);
		expect(
			revealScroll({ index: 0, scrollTop: 60, height: 100, rowHeight: 28, headerHeight: 32 })
		).toBe(0);
		expect(
			revealScroll({ index: 1, scrollTop: 0, height: 100, rowHeight: 28, headerHeight: 32 })
		).toBe(0);
	});
});

describe('createRowWindow refresh', () => {
	it('refresh keeps scroll and re-reads visible rows', async () => {
		const source = memorySource(numbers(1000));
		const win = createRowWindow(source, { rowHeight: 28, overscan: 2, settleMs: 0 });
		await settle();
		win.setViewport(400 * 28, 280);
		await settle();
		const start = win.start;
		source.replace(numbers(1000).map((r) => ({ ...r, name: `new${String(r.id)}` })));
		win.refresh();
		await settle();
		await settle();
		expect(win.start).toBe(start);
		expect(win.rows[2]).toEqual({ id: 400, name: 'new400' });
	});

	it('shrinking list clamps active row and shows no stale rows', async () => {
		const source = memorySource(numbers(500));
		const win = createRowWindow(source, { rowHeight: 28, overscan: 2, settleMs: 0 });
		await settle();
		win.setViewport(400 * 28, 280);
		await settle();
		source.replace(numbers(120));
		win.refresh();
		await settle();
		await settle();
		expect(win.count).toBe(120);
		expect(win.start).toBeLessThan(120);
		expect(win.rows.every((r) => r !== undefined && r.id < 120)).toBe(true);
	});
});
