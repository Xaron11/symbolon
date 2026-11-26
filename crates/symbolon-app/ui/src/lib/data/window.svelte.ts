import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import type { DataSource, SortSpec } from './source';

/** The visible slice of a virtualized list and how to move it. */
export interface RowWindow<Row> {
	/** Total rows, once known. */
	readonly count: number | undefined;
	/** Index of the first row in `rows`. */
	readonly start: number;
	/** The rows from `start` to the end of the window; undefined while a page is loading. */
	readonly rows: readonly (Row | undefined)[];
	/** The last error reading rows, until `retry`. */
	readonly error: Error | undefined;
	readonly sort: SortSpec | undefined;
	setViewport(scrollTop: number, height: number): void;
	setSort(sort: SortSpec | undefined): void;
	retry(): void;
}

/**
 * Keeps the rows around a viewport loaded from `source`, a page at a time. Viewport changes in
 * one task are coalesced into one round of requests for the final position; at most `maxPages`
 * pages stay cached; responses that arrive after the sort changed are dropped.
 */
export function createRowWindow<Row>(
	source: DataSource<Row>,
	options: { rowHeight: number; overscan?: number; pageSize?: number; maxPages?: number }
): RowWindow<Row> {
	const { rowHeight, overscan = 10, pageSize = 200, maxPages = 10 } = options;
	let count = $state<number | undefined>(undefined);
	let error = $state<Error | undefined>(undefined);
	let sort = $state<SortSpec | undefined>(undefined);
	let first = $state(0);
	let last = $state(-1);
	const pages = new SvelteMap<number, readonly Row[]>();
	const loading = new SvelteSet<number>();
	let generation = 0;
	let scheduled = false;
	let viewport = { top: 0, height: 0 };

	function readCount() {
		source.count().then(
			(n) => {
				count = n;
				schedule();
			},
			(e: unknown) => (error = toError(e))
		);
	}

	function schedule() {
		if (scheduled) return;
		scheduled = true;
		queueMicrotask(() => {
			scheduled = false;
			load();
		});
	}

	function load() {
		if (count === undefined) return;
		const total = count;
		first = Math.max(0, Math.floor(viewport.top / rowHeight) - overscan);
		last = Math.min(total - 1, Math.ceil((viewport.top + viewport.height) / rowHeight) + overscan);
		if (last < first) return;
		for (let page = Math.floor(first / pageSize); page <= Math.floor(last / pageSize); page++) {
			if (pages.has(page)) {
				// Mark as recently used.
				const rows = pages.get(page) ?? [];
				pages.delete(page);
				pages.set(page, rows);
				continue;
			}
			if (loading.has(page)) continue;
			loading.add(page);
			const requested = generation;
			source.rows(page * pageSize, pageSize, sort).then(
				(rows) => {
					loading.delete(page);
					if (requested !== generation) return;
					pages.set(page, rows);
					while (pages.size > maxPages) pages.delete(pages.keys().next().value ?? page);
				},
				(e: unknown) => {
					loading.delete(page);
					if (requested === generation) error = toError(e);
				}
			);
		}
	}

	function reset() {
		generation++;
		pages.clear();
		loading.clear();
	}

	readCount();

	return {
		get count() {
			return count;
		},
		get start() {
			return first;
		},
		get rows() {
			const rows: (Row | undefined)[] = [];
			for (let i = first; i <= last; i++)
				rows.push(pages.get(Math.floor(i / pageSize))?.[i % pageSize]);
			return rows;
		},
		get error() {
			return error;
		},
		get sort() {
			return sort;
		},
		setViewport(scrollTop, height) {
			viewport = { top: Math.max(0, scrollTop), height };
			schedule();
		},
		setSort(next) {
			sort = next;
			reset();
			schedule();
		},
		retry() {
			error = undefined;
			reset();
			if (count === undefined) readCount();
			else schedule();
		}
	};
}

function toError(e: unknown): Error {
	return e instanceof Error ? e : new Error(String(e));
}
