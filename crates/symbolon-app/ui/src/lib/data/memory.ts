import type { DataSource, SortSpec, TreeSource } from './source';

/** A request a memory source served, recorded for tests. */
export interface SourceCall {
	offset: number;
	limit: number;
	sort?: SortSpec | undefined;
}

/** A memory source's extras for tests: its calls, and a way to change its rows. */
export interface MemorySourceControl<Row> {
	readonly calls: SourceCall[];
	/** How many subscribers it has. */
	readonly subscribers: number;
	/** Replaces the rows and tells subscribers. */
	replace(rows: readonly Row[]): void;
}

/** Subscribers of a memory source or tree. */
function listeners() {
	const set = new Set<() => void>();
	return {
		set,
		subscribe(onChange: () => void) {
			set.add(onChange);
			return () => set.delete(onChange);
		},
		notify() {
			for (const listener of [...set]) listener();
		}
	};
}

/**
 * A data source over an array, for tests and the gallery. `compare` builds the ordering for a
 * sort; by default rows are compared by the sorted column's text.
 */
export function memorySource<Row>(
	initial: readonly Row[],
	compare?: (sort: SortSpec) => (a: Row, b: Row) => number
): DataSource<Row> & MemorySourceControl<Row> {
	const calls: SourceCall[] = [];
	const changes = listeners();
	let rows = initial;
	const byColumn = (sort: SortSpec) => (a: Row, b: Row) => {
		const text = (row: Row) => String((row as Record<string, unknown>)[sort.column]);
		return (sort.direction === 'asc' ? 1 : -1) * text(a).localeCompare(text(b));
	};
	return {
		calls,
		get subscribers() {
			return changes.set.size;
		},
		replace(next) {
			rows = next;
			changes.notify();
		},
		subscribe: (onChange) => changes.subscribe(onChange),
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

/**
 * A tree source over nested nodes, for tests and the gallery; records `children` calls, and
 * `replace` swaps the nodes and tells subscribers.
 */
export function memoryTree(initial: readonly MemoryNode[]): TreeSource<MemoryNode> & {
	readonly childCalls: string[];
	replace(roots: readonly MemoryNode[]): void;
} {
	const childCalls: string[] = [];
	const changes = listeners();
	let roots = initial;
	// Children by key, so a replaced tree serves the new children of a node read before.
	const byKey = () => {
		const map = new Map<string, MemoryNode>();
		const walk = (list: readonly MemoryNode[]) => {
			for (const node of list) {
				map.set(node.key, node);
				walk(node.children ?? []);
			}
		};
		walk(roots);
		return map;
	};
	const current = (node: MemoryNode) => byKey().get(node.key) ?? node;
	return {
		childCalls,
		replace(next) {
			roots = next;
			changes.notify();
		},
		subscribe: (onChange) => changes.subscribe(onChange),
		rootCount: () => Promise.resolve(roots.length),
		roots: (offset, limit) => Promise.resolve(roots.slice(offset, offset + limit)),
		childCount: (node) => Promise.resolve(current(node).children?.length ?? 0),
		children(node, offset, limit) {
			childCalls.push(node.key);
			return Promise.resolve((current(node).children ?? []).slice(offset, offset + limit));
		},
		key: (node) => node.key,
		hasChildren: (node) => node.children !== undefined
	};
}
