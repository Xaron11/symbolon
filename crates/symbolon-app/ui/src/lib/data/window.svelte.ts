import { SvelteMap } from 'svelte/reactivity';
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
 * The scroll position that brings row `index` fully into view below a sticky header of
 * `headerHeight`, or the current one if it already is.
 */
export function revealScroll(at: {
	index: number;
	scrollTop: number;
	height: number;
	rowHeight: number;
	headerHeight: number;
}): number {
	const top = at.index * at.rowHeight;
	if (top < at.scrollTop) return top;
	const bottom = at.headerHeight + top + at.rowHeight;
	return bottom > at.scrollTop + at.height ? bottom - at.height : at.scrollTop;
}

/**
 * Keeps the rows around a viewport loaded from `source`, a page at a time. Requests wait until
 * the viewport has been still for `settleMs`, so dragging the scrollbar reads only where it
 * stops; answers for pages no longer in view or for an older sort are dropped; at most
 * `maxPages` pages stay cached, never evicting the ones in view.
 */
export function createRowWindow<Row>(
	source: DataSource<Row>,
	options: {
		rowHeight: number;
		overscan?: number;
		pageSize?: number;
		maxPages?: number;
		settleMs?: number;
	}
): RowWindow<Row> {
	const { rowHeight, overscan = 10, pageSize = 200, maxPages = 10, settleMs = 30 } = options;
	let count = $state<number | undefined>(undefined);
	let error = $state<Error | undefined>(undefined);
	let sort = $state<SortSpec | undefined>(undefined);
	let first = $state(0);
	let last = $state(-1);
	const pages = new SvelteMap<number, readonly Row[]>();
	// Page → generation of the request in flight for it.
	const loading = new SvelteMap<number, number>();
	let generation = 0;
	let settle: ReturnType<typeof setTimeout> | undefined;
	let viewport = { top: 0, height: 0 };

	function readCount() {
		source.count().then(
			(n) => {
				count = n;
				place();
				schedule();
			},
			(e: unknown) => (error = toError(e))
		);
	}

	function schedule() {
		clearTimeout(settle);
		settle = setTimeout(load, settleMs);
	}

	/** The pages the current window needs. */
	const needed = () => ({ from: Math.floor(first / pageSize), to: Math.floor(last / pageSize) });

	// The window follows the viewport at once (cached pages show immediately); only reading
	// missing pages waits for the viewport to settle.
	function place() {
		if (count === undefined) return;
		first = Math.max(0, Math.floor(viewport.top / rowHeight) - overscan);
		last = Math.min(count - 1, Math.ceil((viewport.top + viewport.height) / rowHeight) + overscan);
	}

	function load() {
		if (count === undefined || last < first) return;
		for (let page = Math.floor(first / pageSize); page <= Math.floor(last / pageSize); page++) {
			if (pages.has(page)) {
				// Mark as recently used.
				const rows = pages.get(page) ?? [];
				pages.delete(page);
				pages.set(page, rows);
				continue;
			}
			if (loading.get(page) === generation) continue;
			const requested = generation;
			loading.set(page, requested);
			const done = () => {
				if (loading.get(page) === requested) loading.delete(page);
			};
			source.rows(page * pageSize, pageSize, sort).then(
				(rows) => {
					done();
					const range = needed();
					if (requested !== generation || page < range.from || page > range.to) return;
					pages.set(page, rows);
					for (const old of pages.keys()) {
						if (pages.size <= maxPages) break;
						if (old < range.from || old > range.to) pages.delete(old);
					}
				},
				(e: unknown) => {
					done();
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
			place();
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
