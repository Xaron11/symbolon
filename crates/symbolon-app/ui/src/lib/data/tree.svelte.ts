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
	/** Re-reads the tree after the source changed, keeping open the folders that still exist. */
	refresh(): Promise<void>;
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
	// Bumped by every refresh; folder reads started before it are dropped.
	let generation = 0;
	const fail = (e: unknown) => {
		error = e instanceof Error ? e : new Error(String(e));
	};

	const remember = (list: readonly Node[]) => {
		for (const node of list) nodes.set(source.key(node), node);
		return list;
	};

	const readRoots = () =>
		source
			.rootCount()
			.then((count) => readAll(count, (o, l) => source.roots(o, l)))
			.then((list) => (roots = remember(list)), fail);
	void readRoots();

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
				const started = generation;
				read = source
					.childCount(node)
					.then((count) => readAll(count, (o, l) => source.children(node, o, l)))
					.then((list) => {
						if (started === generation) children.set(key, remember(list));
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

	let refreshing: Promise<void> | undefined;
	let queued = false;
	// Read through a call: `queued` changes while a rebuild is awaited.
	const isQueued = () => queued;

	/**
	 * Reads the tree again beside the one shown, reopening the open folders that still exist
	 * (parents before children, whatever order they were opened in), then swaps it in at once
	 * so the view never shows a half-read tree.
	 */
	async function rebuild() {
		const started = ++generation;
		const open = [...expanded];
		const freshNodes = new SvelteMap<string, Node>();
		const freshChildren = new SvelteMap<string, readonly Node[]>();
		const keep = (list: readonly Node[]) => {
			for (const node of list) freshNodes.set(source.key(node), node);
			return list;
		};
		try {
			const freshRoots = keep(
				await readAll(await source.rootCount(), (o, l) => source.roots(o, l))
			);
			let progress = true;
			while (progress) {
				progress = false;
				for (const key of open) {
					const node = freshNodes.get(key);
					if (freshChildren.has(key) || node === undefined || !source.hasChildren(node)) continue;
					const list = await readAll(await source.childCount(node), (o, l) =>
						source.children(node, o, l)
					);
					freshChildren.set(key, keep(list));
					progress = true;
				}
			}
			if (started !== generation) return;
			roots = freshRoots;
			nodes.clear();
			for (const [key, node] of freshNodes) nodes.set(key, node);
			children.clear();
			for (const [key, list] of freshChildren) children.set(key, list);
			reading.clear();
			for (const key of open) {
				if (!freshChildren.has(key)) {
					expanded.delete(key);
					wanted.delete(key);
				}
			}
			error = undefined;
		} catch (e) {
			fail(e);
		}
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
		},
		refresh() {
			// One refresh at a time; changes during it ask for one more, not one each.
			if (refreshing) {
				queued = true;
				return refreshing;
			}
			refreshing = (async () => {
				do {
					queued = false;
					await rebuild();
				} while (isQueued());
			})().finally(() => {
				refreshing = undefined;
			});
			return refreshing;
		}
	};
}
