import type { Backend } from './backend';
import { ApiError } from './errors';
import type {
	BrowseChange,
	CompareRowView,
	DeviceView,
	DuplicateSetView,
	DuplicateTotalsView,
	EntryView,
	ExtensionView,
	LocationView,
	RecentScanView,
	ScanUpdate,
	SizeBucketView,
	SummaryView,
	TreeNodeView,
	VerifyUpdate,
	WarningView
} from './types';

/** What a comparison opened on a memory backend shows. */
export interface MemoryComparison {
	summary: SummaryView;
	rows: CompareRowView[];
	/** Children by parent key (`''` for the root). */
	tree?: Record<string, TreeNodeView[]>;
	extensions?: ExtensionView[];
	sizes?: SizeBucketView[];
	/** Duplicate groups per side. */
	duplicates?: DuplicateSetView[][];
	duplicateTotals?: DuplicateTotalsView[];
}

/** The data a memory backend serves; tests change it between calls. */
export interface MemorySeed {
	locations?: LocationView[];
	devices?: DeviceView[];
	recentScans?: RecentScanView[];
	/** Warnings by scan id. */
	warnings?: Record<number, WarningView[]>;
	/** Entries by location label, then by parent key (`''` for the root). */
	entries?: Record<string, Record<string, EntryView[]>>;
	comparison?: MemoryComparison;
	/** What the folder picker returns; undefined means cancelled. */
	pickedFolder?: string;
}

/** A call a memory backend served, recorded for tests. */
export interface BackendCall {
	method: string;
	args: unknown[];
}

/** A {@link Backend} over memory, for tests and the gallery, with ways to play the backend's part. */
export interface MemoryBackend extends Backend {
	readonly seed: MemorySeed;
	readonly calls: readonly BackendCall[];
	/** Sends `update` to whoever follows the scan of `label`; a final update ends the scan. */
	emitScan(label: string, update: ScanUpdate): void;
	/** Sends `update` to the opener of comparison `id`; `ResultsChanged` also updates its totals. */
	emitVerify(id: number, update: VerifyUpdate): void;
	/** Sends `change` to the browsers of `label`. */
	emitBrowse(label: string, change: BrowseChange): void;
}

const notFound = (what: string) => new ApiError('NotFound', `${what} not found`);

/** A backend serving `seed` from memory. */
export function createMemoryBackend(seed: MemorySeed = {}): MemoryBackend {
	const calls: BackendCall[] = [];
	const record = (method: string, ...args: unknown[]) => {
		calls.push({ method, args });
	};
	const scanListeners = new Map<string, ((update: ScanUpdate) => void)[]>();
	const latest = new Map<string, ScanUpdate>();
	const browsers = new Map<string, Set<(change: BrowseChange) => void>>();
	const open = new Map<number, (update: VerifyUpdate) => void>();
	let nextId = 1;

	const location = (label: string) => {
		const found = (seed.locations ??= []).find((l) => l.label === label);
		if (!found) throw notFound(`location ${label}`);
		return found;
	};
	const comparison = (id: number) => {
		if (!open.has(id) || !seed.comparison) throw notFound(`comparison ${String(id)}`);
		return seed.comparison;
	};
	const page = <T>(items: readonly T[], offset: number, limit: number) =>
		items.slice(offset, offset + limit);
	const resolve = <T>(work: () => T): Promise<T> => {
		try {
			return Promise.resolve(work());
		} catch (error) {
			return Promise.reject(error instanceof Error ? error : new Error(String(error)));
		}
	};

	const backend: MemoryBackend = {
		seed,
		calls,
		system: {
			pickFolder: () => resolve(() => (record('system.pickFolder'), seed.pickedFolder))
		},
		locations: {
			list: () => resolve(() => (record('locations.list'), [...(seed.locations ?? [])])),
			add: (request) =>
				resolve(() => {
					record('locations.add', request);
					const list = (seed.locations ??= []);
					if (list.some((l) => l.label === request.label)) {
						throw new ApiError(
							'AlreadyExists',
							`a location labelled "${request.label}" already exists`
						);
					}
					const view: LocationView = {
						label: request.label,
						root:
							request.kind.type === 'Folder'
								? request.kind.path
								: `android:${request.kind.serial}:${request.kind.path}`,
						kind: request.kind.type,
						status: { type: 'Connected' },
						hashPolicy: request.hashPolicy,
						hashAlgo: request.hashAlgo ?? 'blake3',
						lastScan: null,
						stats: { files: 0, dirs: 0, symlinks: 0, bytes: 0, verifiedFiles: 0, partialFiles: 0 }
					};
					list.push(view);
					return view;
				}),
			rename: (label, newLabel) =>
				resolve(() => {
					record('locations.rename', label, newLabel);
					const found = location(label);
					found.label = newLabel;
					return found;
				}),
			remove: (label) =>
				resolve(() => {
					record('locations.remove', label);
					location(label);
					if (scanListeners.has(label))
						throw new ApiError('AlreadyScanning', 'Stop the scan first.');
					seed.locations = (seed.locations ?? []).filter((l) => l.label !== label);
				}),
			setHashPolicy: (label, policy) =>
				resolve(() => {
					record('locations.setHashPolicy', label, policy);
					const found = location(label);
					found.hashPolicy = policy;
					return found;
				}),
			followMove: (label, choice) =>
				resolve(() => {
					record('locations.followMove', label, choice);
					const found = location(label);
					found.status = { type: 'Connected' };
					return found;
				}),
			stats: (label) => resolve(() => (record('locations.stats', label), location(label).stats))
		},
		devices: {
			list: () => resolve(() => (record('devices.list'), [...(seed.devices ?? [])]))
		},
		scans: {
			start: (label, onUpdate) =>
				resolve(() => {
					record('scans.start', label);
					location(label);
					if (scanListeners.has(label)) {
						throw new ApiError('AlreadyScanning', `location "${label}" is already being scanned`);
					}
					scanListeners.set(label, [onUpdate]);
				}),
			attach: (label, onUpdate) =>
				resolve(() => {
					record('scans.attach', label);
					const listeners = scanListeners.get(label);
					if (!listeners) throw notFound(`scan of ${label}`);
					listeners.push(onUpdate);
					const last = latest.get(label);
					if (last) onUpdate(last);
				}),
			running: () =>
				resolve(() => {
					record('scans.running');
					return [...scanListeners.keys()].flatMap((label) => latest.get(label) ?? []);
				}),
			pause: (label) =>
				resolve(() => {
					record('scans.pause', label);
				}),
			resume: (label) =>
				resolve(() => {
					record('scans.resume', label);
				}),
			cancel: (label) =>
				resolve(() => {
					record('scans.cancel', label);
				}),
			recent: (offset, limit) =>
				resolve(
					() => (record('scans.recent', offset, limit), page(seed.recentScans ?? [], offset, limit))
				),
			recentCount: () => resolve(() => (seed.recentScans ?? []).length),
			get: (scanId) =>
				resolve(() => (seed.recentScans ?? []).find((r) => r.scan.scanId === scanId)),
			ofLocation: (label, offset, limit) =>
				resolve(() =>
					page(
						(seed.recentScans ?? []).filter((r) => r.label === label).map((r) => r.scan),
						offset,
						limit
					)
				),
			countOfLocation: (label) =>
				resolve(() => (seed.recentScans ?? []).filter((r) => r.label === label).length),
			warnings: (scanId, offset, limit) =>
				resolve(() => page(seed.warnings?.[scanId] ?? [], offset, limit)),
			warningCount: (scanId) => resolve(() => (seed.warnings?.[scanId] ?? []).length)
		},
		browse: {
			childCount: (label, parent) => resolve(() => (seed.entries?.[label]?.[parent] ?? []).length),
			children: (label, parent, offset, limit) =>
				resolve(() => {
					record('browse.children', label, parent, offset, limit);
					return page(seed.entries?.[label]?.[parent] ?? [], offset, limit);
				}),
			subscribe(label, onChange) {
				record('browse.subscribe', label);
				const set = browsers.get(label) ?? new Set();
				set.add(onChange);
				browsers.set(label, set);
				return () => set.delete(onChange);
			}
		},
		compare: {
			open: (request, onUpdate) =>
				resolve(() => {
					record('compare.open', request);
					if (!seed.comparison) throw new ApiError('InvalidInput', 'nothing to compare');
					const id = nextId++;
					open.set(id, onUpdate);
					return {
						id,
						sides: request.sides.map((side) => ({
							label: side.label,
							subpath: side.subpath ?? null
						})),
						summary: seed.comparison.summary
					};
				}),
			close: (id) =>
				resolve(() => {
					record('compare.close', id);
					if (!open.delete(id)) throw notFound(`comparison ${String(id)}`);
				}),
			closeAll: () =>
				resolve(() => {
					record('compare.closeAll');
					open.clear();
				}),
			verify: (id) => resolve(() => void (record('compare.verify', id), comparison(id))),
			cancelVerify: (id) =>
				resolve(() => void (record('compare.cancelVerify', id), comparison(id))),
			summary: (id) => resolve(() => comparison(id).summary),
			count: (id, filter) =>
				resolve(() => {
					record('compare.count', id, filter);
					return filtered(comparison(id).rows, filter.categories).length;
				}),
			rows: (id, filter, order, offset, limit) =>
				resolve(() => {
					record('compare.rows', id, filter, order, offset, limit);
					const rows = [...filtered(comparison(id).rows, filter.categories)].sort((a, b) =>
						order.by === 'Size' ? (a.size ?? -1) - (b.size ?? -1) : a.key.localeCompare(b.key)
					);
					if (order.desc) rows.reverse();
					return page(rows, offset, limit);
				}),
			childCount: (id, filter, parent) =>
				resolve(
					() => (
						record('compare.childCount', id, filter, parent),
						(comparison(id).tree?.[parent] ?? []).length
					)
				),
			children: (id, filter, parent, offset, limit) =>
				resolve(() => {
					record('compare.children', id, filter, parent, offset, limit);
					return page(comparison(id).tree?.[parent] ?? [], offset, limit);
				}),
			byExtension: (id) => resolve(() => comparison(id).extensions ?? []),
			sizeHistogram: (id) => resolve(() => comparison(id).sizes ?? []),
			duplicates: (id, side, offset, limit) =>
				resolve(() => page(comparison(id).duplicates?.[side] ?? [], offset, limit)),
			duplicateTotals: (id) => resolve(() => comparison(id).duplicateTotals ?? [])
		},
		emitScan(label, update) {
			latest.set(label, update);
			for (const listener of scanListeners.get(label) ?? []) listener(update);
			if (update.type === 'Finished' || update.type === 'Failed') {
				scanListeners.delete(label);
				latest.delete(label);
			}
		},
		emitVerify(id, update) {
			if (update.type === 'ResultsChanged' && seed.comparison) {
				seed.comparison.summary = update.summary;
			}
			open.get(id)?.(update);
		},
		emitBrowse(label, change) {
			for (const listener of browsers.get(label) ?? []) listener(change);
		}
	};
	return backend;
}

function filtered(rows: readonly CompareRowView[], categories: readonly string[] | undefined) {
	return categories && categories.length > 0
		? rows.filter((row) => categories.includes(row.category))
		: rows;
}
