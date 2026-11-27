import { describe, expect, it, vi } from 'vitest';
import { createMemoryBackend } from './memory';
import { catalogTree, comparisonRows, comparisonTree, createChangeHub, rowOrder } from './sources';
import type { CompareRowView, EntryView, TreeNodeView } from './types';

function row(key: string, category: CompareRowView['category'], size: number): CompareRowView {
	return { key, display: key, category, presence: [0, 1], assumed: false, size, cells: [] };
}

function entry(key: string, folder: boolean): EntryView {
	return {
		key,
		name: key.split('/').at(-1) ?? key,
		kind: folder ? 'Dir' : 'File',
		folder,
		size: 1,
		mtimeMs: null,
		hashState: 'Unhashed'
	};
}

async function opened(rows: CompareRowView[], tree: Record<string, TreeNodeView[]> = {}) {
	const b = createMemoryBackend({ comparison: { summary: { categories: [] }, rows, tree } });
	const { id } = await b.compare.open(
		{
			sides: [{ label: 'a' }, { label: 'b' }],
			nfc: false,
			caseInsensitive: false,
			duplicates: false
		},
		() => undefined
	);
	return { b, id };
}

describe('data sources', () => {
	it('comparisonRows count/rows map to backend calls', async () => {
		const { b, id } = await opened([row('a', 'different', 1), row('b', 'identical', 2)]);
		const source = comparisonRows(b, id, { categories: ['different'] });
		await expect(source.count()).resolves.toBe(1);
		await expect(source.rows(0, 10)).resolves.toEqual([row('a', 'different', 1)]);
		expect(b.calls.at(-1)).toEqual({
			method: 'compare.rows',
			args: [id, { categories: ['different'] }, { by: 'Path', desc: false }, 0, 10]
		});
	});

	it('size sort maps to order { by: "Size", desc }', () => {
		expect(rowOrder({ column: 'size', direction: 'desc' })).toEqual({ by: 'Size', desc: true });
		expect(rowOrder({ column: 'path', direction: 'asc' })).toEqual({ by: 'Path', desc: false });
		expect(rowOrder(undefined)).toEqual({ by: 'Path', desc: false });
	});

	it('subscribe fires on ResultsChanged', async () => {
		const { b, id } = await opened([]);
		const hub = createChangeHub();
		const source = comparisonRows(b, id, {}, hub);
		const onChange = vi.fn();
		const stop = source.subscribe?.(onChange);
		hub.notify();
		expect(onChange).toHaveBeenCalledTimes(1);
		stop?.();
		hub.notify();
		expect(onChange).toHaveBeenCalledTimes(1);
	});

	it('sources without a hub do not subscribe', async () => {
		const { b, id } = await opened([]);
		expect('subscribe' in comparisonRows(b, id, {})).toBe(false);
	});

	it('comparisonTree reads children by folder key', async () => {
		const folder: TreeNodeView = {
			key: 'docs',
			name: 'docs',
			folder: true,
			row: null,
			matchingRows: 1,
			matchingBytes: 5
		};
		const { b, id } = await opened([], { '': [folder], docs: [] });
		const tree = comparisonTree(b, id, {});
		await expect(tree.roots(0, 10)).resolves.toEqual([folder]);
		expect(tree.hasChildren(folder)).toBe(true);
		await tree.children(folder, 0, 10);
		expect(b.calls.at(-1)?.args[2]).toBe('docs');
	});

	it('catalogTree children use the folder key', async () => {
		const odd = entry('dir%E9', true);
		const b = createMemoryBackend({
			entries: { loc: { '': [odd], 'dir%E9': [entry('dir%E9/x', false)] } }
		});
		const tree = catalogTree(b, 'loc');
		const [root] = await tree.roots(0, 10);
		expect(root).toEqual(odd);
		await expect(tree.children(odd, 0, 10)).resolves.toHaveLength(1);
		expect(b.calls.at(-1)).toEqual({
			method: 'browse.children',
			args: ['loc', 'dir%E9', 0, 10]
		});
		const onChange = vi.fn();
		const stop = tree.subscribe?.(onChange);
		b.emitBrowse('loc', 'Changed');
		expect(onChange).toHaveBeenCalledTimes(1);
		stop?.();
		b.emitBrowse('loc', 'Changed');
		expect(onChange).toHaveBeenCalledTimes(1);
	});
});
