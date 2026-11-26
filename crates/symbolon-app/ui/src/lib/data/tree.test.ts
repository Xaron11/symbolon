import { describe, expect, it } from 'vitest';
import { memoryTree } from './memory';
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
});
