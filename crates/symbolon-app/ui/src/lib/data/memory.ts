import type { DataSource, SortSpec, TreeSource } from './source';

/** A request a memory source served, recorded for tests. */
export interface SourceCall {
	offset: number;
	limit: number;
	sort?: SortSpec | undefined;
}

/**
 * A data source over an array, for tests and the gallery. `compare` builds the ordering for a
 * sort; by default rows are compared by the sorted column's text.
 */
export function memorySource<Row>(
	rows: readonly Row[],
	compare?: (sort: SortSpec) => (a: Row, b: Row) => number
): DataSource<Row> & { readonly calls: SourceCall[] } {
	const calls: SourceCall[] = [];
	const byColumn = (sort: SortSpec) => (a: Row, b: Row) => {
		const text = (row: Row) => String((row as Record<string, unknown>)[sort.column]);
		return (sort.direction === 'asc' ? 1 : -1) * text(a).localeCompare(text(b));
	};
	return {
		calls,
		count: () => Promise.resolve(rows.length),
		rows(offset, limit, sort) {
			calls.push({ offset, limit, sort });
			const ordered = sort ? [...rows].sort((compare ?? byColumn)(sort)) : rows;
			return Promise.resolve(ordered.slice(offset, offset + limit));
		}
	};
}

/** A node of a memory tree. */
export interface MemoryNode {
	key: string;
	label: string;
	children?: readonly MemoryNode[];
}

/** A tree source over nested nodes, for tests and the gallery; records `children` calls. */
export function memoryTree(
	roots: readonly MemoryNode[]
): TreeSource<MemoryNode> & { readonly childCalls: string[] } {
	const childCalls: string[] = [];
	return {
		childCalls,
		rootCount: () => Promise.resolve(roots.length),
		roots: (offset, limit) => Promise.resolve(roots.slice(offset, offset + limit)),
		childCount: (node) => Promise.resolve(node.children?.length ?? 0),
		children(node, offset, limit) {
			childCalls.push(node.key);
			return Promise.resolve((node.children ?? []).slice(offset, offset + limit));
		},
		key: (node) => node.key,
		hasChildren: (node) => node.children !== undefined
	};
}
