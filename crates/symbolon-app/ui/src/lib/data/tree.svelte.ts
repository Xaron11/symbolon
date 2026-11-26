import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import type { TreeSource } from './source';

/** A node as shown: its depth and its place among its siblings. */
export interface VisibleNode<Node> {
	readonly node: Node;
	readonly key: string;
	/** 1 for roots. */
	readonly level: number;
	readonly expanded: boolean;
	readonly hasChildren: boolean;
	readonly setSize: number;
	readonly posInSet: number;
	/** Key of the parent, undefined for roots. */
	readonly parent: string | undefined;
}

/** The expanded state of a tree and the flat list of what is visible. */
export interface TreeState<Node> {
	readonly visible: readonly VisibleNode<Node>[];
	/** The last failed read, if any. */
	readonly error: Error | undefined;
	expand(key: string): Promise<void>;
	collapse(key: string): void;
	toggle(key: string): Promise<void>;
}

const PAGE = 500;

async function readAll<T>(
	count: number,
	page: (offset: number, limit: number) => Promise<readonly T[]>
) {
	const all: T[] = [];
	for (let offset = 0; offset < count; offset += PAGE) all.push(...(await page(offset, PAGE)));
	return all;
}

/**
 * Reads the roots of `source` and each folder's children the first time it is expanded;
 * collapsing keeps them, so expanding again reads nothing.
 */
export function createTreeState<Node>(source: TreeSource<Node>): TreeState<Node> {
	let roots = $state.raw<readonly Node[]>([]);
	const children = new SvelteMap<string, readonly Node[]>();
	const expanded = new SvelteSet<string>();
	const nodes = new SvelteMap<string, Node>();
	// Folders the user wants open, and the reads in flight, so a folder is read once and a
	// collapse during its read wins.
	const wanted = new SvelteSet<string>();
	const reading = new SvelteMap<string, Promise<void>>();
	let error = $state<Error | undefined>(undefined);
	const fail = (e: unknown) => {
		error = e instanceof Error ? e : new Error(String(e));
	};

	const remember = (list: readonly Node[]) => {
		for (const node of list) nodes.set(source.key(node), node);
		return list;
	};

	void source
		.rootCount()
		.then((count) => readAll(count, (o, l) => source.roots(o, l)))
		.then((list) => (roots = remember(list)), fail);

	const visible = $derived.by(() => {
		const out: VisibleNode<Node>[] = [];
		const walk = (list: readonly Node[], level: number, parent: string | undefined) => {
			list.forEach((node, i) => {
				const key = source.key(node);
				const open = expanded.has(key);
				out.push({
					node,
					key,
					level,
					expanded: open,
					hasChildren: source.hasChildren(node),
					setSize: list.length,
					posInSet: i + 1,
					parent
				});
				const kids = children.get(key);
				if (open && kids) walk(kids, level + 1, key);
			});
		};
		walk(roots, 1, undefined);
		return out;
	});

	async function expand(key: string) {
		const node = nodes.get(key);
		if (node === undefined || !source.hasChildren(node)) return;
		wanted.add(key);
		if (!children.has(key)) {
			let read = reading.get(key);
			if (read === undefined) {
				read = source
					.childCount(node)
					.then((count) => readAll(count, (o, l) => source.children(node, o, l)))
					.then((list) => {
						children.set(key, remember(list));
					}, fail)
					.finally(() => reading.delete(key));
				reading.set(key, read);
			}
			await read;
		}
		if (wanted.has(key) && children.has(key)) expanded.add(key);
	}

	function collapse(key: string) {
		wanted.delete(key);
		expanded.delete(key);
	}

	return {
		get visible() {
			return visible;
		},
		get error() {
			return error;
		},
		expand,
		collapse,
		async toggle(key) {
			if (expanded.has(key)) collapse(key);
			else await expand(key);
		}
	};
}
