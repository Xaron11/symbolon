import type { DataSource, SortSpec, TreeSource } from '../data/source';
import type { Backend } from './backend';
import type {
	ApiFilter,
	CompareRowView,
	DuplicateSetView,
	EntryView,
	RecentScanView,
	RowOrder,
	ScanBrief,
	TreeNodeView,
	WarningView
} from './types';

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

/** The scans of location `label`, newest first, for a table. */
export function locationScans(backend: Backend, label: string): DataSource<ScanBrief> {
	return {
		count: () => backend.scans.countOfLocation(label),
		rows: (offset, limit) => backend.scans.ofLocation(label, offset, limit)
	};
}

/** The warnings of scan `scanId`, in the order they were found, for a table. */
export function scanWarnings(backend: Backend, scanId: number): DataSource<WarningView> {
	return {
		count: () => backend.scans.warningCount(scanId),
		rows: (offset, limit) => backend.scans.warnings(scanId, offset, limit)
	};
}

/** Scans of every location, newest first, for a table. */
export function recentScans(backend: Backend): DataSource<RecentScanView> {
	return {
		count: () => backend.scans.recentCount(),
		rows: (offset, limit) => backend.scans.recent(offset, limit)
	};
}

/** A duplicate group of one side, or one of its copies. */
export type DuplicateNode =
	| { kind: 'group'; set: DuplicateSetView }
	| { kind: 'copy'; digest: string; index: number; path: string };

/**
 * The duplicate groups of side `side` as a tree: groups by page from the backend, each with its
 * copies as children; read again after every verification round.
 */
export function duplicateGroups(
	backend: Backend,
	id: number,
	side: number,
	changes?: ChangeHub
): TreeSource<DuplicateNode> {
	return {
		rootCount: async () => (await backend.compare.duplicateTotals(id))[side]?.groups ?? 0,
		roots: async (offset, limit) =>
			(await backend.compare.duplicates(id, side, offset, limit)).map((set) => ({
				kind: 'group' as const,
				set
			})),
		childCount: (node) => Promise.resolve(node.kind === 'group' ? node.set.cells.length : 0),
		children: (node, offset, limit) =>
			Promise.resolve(
				node.kind === 'group'
					? node.set.cells.slice(offset, offset + limit).map((cell, i) => ({
							kind: 'copy' as const,
							digest: node.set.digestShort,
							index: offset + i,
							path: cell.path
						}))
					: []
			),
		key: (node) =>
			node.kind === 'group' ? node.set.digestShort : `${node.digest}/${String(node.index)}`,
		hasChildren: (node) => node.kind === 'group',
		...(changes && { subscribe: (onChange: () => void) => changes.subscribe(onChange) })
	};
}
