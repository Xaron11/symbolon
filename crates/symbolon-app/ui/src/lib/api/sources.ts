import type { DataSource, SortSpec, TreeSource } from '../data/source';
import type { Backend } from './backend';
import type { ApiFilter, CompareRowView, EntryView, RowOrder, TreeNodeView } from './types';

/** Tells subscribers that something changed: fed by a comparison's verification updates. */
export interface ChangeHub {
	notify(): void;
	subscribe(onChange: () => void): () => void;
}

/** A {@link ChangeHub} with no subscribers yet. */
export function createChangeHub(): ChangeHub {
	const listeners = new Set<() => void>();
	return {
		notify() {
			for (const listener of [...listeners]) listener();
		},
		subscribe(onChange) {
			listeners.add(onChange);
			return () => listeners.delete(onChange);
		}
	};
}

/** The backend order for a table's sort: column `size` sorts by size, anything else by path. */
export function rowOrder(sort: SortSpec | undefined): RowOrder {
	return {
		by: sort?.column === 'size' ? 'Size' : 'Path',
		desc: sort?.direction === 'desc'
	};
}

/** The rows of comparison `id` matching `filter`, for a table; `changes` signals refreshes. */
export function comparisonRows(
	backend: Backend,
	id: number,
	filter: ApiFilter,
	changes?: ChangeHub
): DataSource<CompareRowView> {
	return {
		count: () => backend.compare.count(id, filter),
		rows: (offset, limit, sort) => backend.compare.rows(id, filter, rowOrder(sort), offset, limit),
		...(changes && { subscribe: (onChange: () => void) => changes.subscribe(onChange) })
	};
}

/** The folder tree of comparison `id` matching `filter`; `changes` signals refreshes. */
export function comparisonTree(
	backend: Backend,
	id: number,
	filter: ApiFilter,
	changes?: ChangeHub
): TreeSource<TreeNodeView> {
	return {
		rootCount: () => backend.compare.childCount(id, filter, ''),
		roots: (offset, limit) => backend.compare.children(id, filter, '', offset, limit),
		childCount: (node) => backend.compare.childCount(id, filter, node.key),
		children: (node, offset, limit) =>
			backend.compare.children(id, filter, node.key, offset, limit),
		key: (node) => node.key,
		hasChildren: (node) => node.folder,
		...(changes && { subscribe: (onChange: () => void) => changes.subscribe(onChange) })
	};
}

/** The catalog of location `label` as a tree, refreshed while it is being scanned. */
export function catalogTree(backend: Backend, label: string): TreeSource<EntryView> {
	return {
		rootCount: () => backend.browse.childCount(label, ''),
		roots: (offset, limit) => backend.browse.children(label, '', offset, limit),
		childCount: (node) => backend.browse.childCount(label, node.key),
		children: (node, offset, limit) => backend.browse.children(label, node.key, offset, limit),
		key: (node) => node.key,
		hasChildren: (node) => node.folder,
		subscribe: (onChange) =>
			backend.browse.subscribe(label, () => {
				onChange();
			})
	};
}
