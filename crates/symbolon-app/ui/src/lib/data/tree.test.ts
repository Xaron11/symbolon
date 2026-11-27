import { describe, expect, it, vi } from 'vitest';
import { memoryTree, type MemoryNode } from './memory';
import { createTreeState } from './tree.svelte';

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const tree = () =>
	memoryTree([
		{
			key: 'a',
			label: 'a',
			children: [
				{ key: 'a/x', label: 'x' },
				{ key: 'a/y', label: 'y', children: [{ key: 'a/y/z', label: 'z' }] }
			]
		},
		{ key: 'b.txt', label: 'b.txt' }
	]);

describe('createTreeState', () => {
	it('lists roots, then children after a first expand only', async () => {
		const source = tree();
		const state = createTreeState(source);
		await settle();
		expect(state.visible.map((v) => v.node.key)).toEqual(['a', 'b.txt']);
		await state.expand('a');
		expect(state.visible.map((v) => [v.node.key, v.level, v.posInSet, v.setSize])).toEqual([
			['a', 1, 1, 2],
			['a/x', 2, 1, 2],
			['a/y', 2, 2, 2],
			['b.txt', 1, 2, 2]
		]);
		state.collapse('a');
		await state.expand('a');
		expect(source.childCalls).toEqual(['a']);
	});

	it('toggles', async () => {
		const state = createTreeState(tree());
		await settle();
		await state.toggle('a');
		expect(state.visible.find((v) => v.node.key === 'a')?.expanded).toBe(true);
		await state.toggle('a');
		expect(state.visible.map((v) => v.node.key)).toEqual(['a', 'b.txt']);
	});

	it('reports a failed read instead of staying silently empty', async () => {
		const state = createTreeState({
			...tree(),
			rootCount: () => Promise.reject(new Error('gone'))
		});
		await settle();
		expect(state.error?.message).toBe('gone');
	});

	it('reads a folder once even when expanded twice quickly', async () => {
		const source = tree();
		const state = createTreeState(source);
		await settle();
		await Promise.all([state.expand('a'), state.expand('a')]);
		expect(source.childCalls).toEqual(['a']);
	});

	it('stays collapsed when collapsed while loading', async () => {
		const state = createTreeState(tree());
		await settle();
		const loading = state.expand('a');
		state.collapse('a');
		await loading;
		expect(state.visible.map((v) => v.key)).toEqual(['a', 'b.txt']);
	});
});

describe('createTreeState refresh', () => {
	it('tree refresh keeps expanded folders that still exist', async () => {
		const source = tree();
		const state = createTreeState(source);
		await settle();
		await state.expand('a');
		await state.expand('a/y');
		source.replace([
			{
				key: 'a',
				label: 'a',
				children: [
					{ key: 'a/w', label: 'w' },
					{ key: 'a/x', label: 'x' }
				]
			},
			{ key: 'c.txt', label: 'c.txt' }
		]);
		await state.refresh();
		expect(state.visible.map((v) => v.node.key)).toEqual(['a', 'a/w', 'a/x', 'c.txt']);
		expect(state.visible[0]?.expanded).toBe(true);
	});
});

/** `tree()`, with every read delayed until `release()`. */
function slowTree() {
	const inner = tree();
	const waiting: (() => void)[] = [];
	const later = <T>(value: Promise<T>) =>
		new Promise<T>((resolve) => waiting.push(() => void value.then(resolve)));
	let slow = false;
	const source = {
		...inner,
		rootCount: () => (slow ? later(inner.rootCount()) : inner.rootCount()),
		roots: (o: number, l: number) => (slow ? later(inner.roots(o, l)) : inner.roots(o, l)),
		childCount: (n: MemoryNode) => (slow ? later(inner.childCount(n)) : inner.childCount(n)),
		children: (n: MemoryNode, o: number, l: number) =>
			slow ? later(inner.children(n, o, l)) : inner.children(n, o, l)
	};
	return {
		source,
		inner,
		slowDown: () => (slow = true),
		release: async () => {
			while (waiting.length > 0) {
				waiting.splice(0).forEach((go) => {
					go();
				});
				await settle();
			}
		}
	};
}

describe('createTreeState refresh, while it reads', () => {
	it('keeps showing the open tree until the new one is read', async () => {
		const t = slowTree();
		const state = createTreeState(t.source);
		await settle();
		await state.expand('a');
		const before = state.visible.map((v) => v.key);
		t.slowDown();
		const refreshed = state.refresh();
		await settle();
		expect(state.visible.map((v) => v.key)).toEqual(before);
		await t.release();
		await refreshed;
		expect(state.visible.map((v) => v.key)).toEqual(before);
	});

	it('coalesces refreshes that arrive while one runs', async () => {
		const t = slowTree();
		const state = createTreeState(t.source);
		await settle();
		const rootCount = vi.spyOn(t.source, 'rootCount');
		t.slowDown();
		const all = Promise.all([state.refresh(), state.refresh(), state.refresh()]);
		await t.release();
		await all;
		expect(rootCount).toHaveBeenCalledTimes(2);
	});

	it('reopens folders whatever order they were opened in', async () => {
		const state = createTreeState(tree());
		await settle();
		await state.expand('a');
		await state.expand('a/y');
		state.collapse('a');
		await state.expand('a');
		await state.refresh();
		expect(state.visible.map((v) => v.key)).toEqual(['a', 'a/x', 'a/y', 'a/y/z', 'b.txt']);
	});
});

describe('createTreeState loading', () => {
	it('is loading until the roots arrive', async () => {
		const state = createTreeState(tree());
		expect(state.loading).toBe(true);
		await settle();
		expect(state.loading).toBe(false);
	});
});
