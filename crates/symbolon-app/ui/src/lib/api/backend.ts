import type {
	ApiFilter,
	BrowseChange,
	CompareOpened,
	CompareRequest,
	CompareRowView,
	DeviceView,
	DuplicateSetView,
	DuplicateTotalsView,
	EntryView,
	ExtensionView,
	HashPolicy,
	LocationView,
	NewLocationRequest,
	RecentScanView,
	RowOrder,
	ScanBrief,
	ScanUpdate,
	SizeBucketView,
	StatsView,
	SummaryView,
	TreeNodeView,
	VerifyUpdate,
	WarningView
} from './types';

/**
 * Everything screens ask of Symbolon (ADR-0021 §2). One implementation per transport: Tauri
 * commands now, HTTP later, memory in tests. Every method rejects with an `ApiError`.
 */
export interface Backend {
	readonly locations: {
		list(): Promise<LocationView[]>;
		add(request: NewLocationRequest): Promise<LocationView>;
		rename(label: string, newLabel: string): Promise<LocationView>;
		remove(label: string): Promise<void>;
		setHashPolicy(label: string, policy: HashPolicy): Promise<LocationView>;
		/** Points a moved location at its device's new place, or at candidate `choice`. */
		followMove(label: string, choice?: number): Promise<LocationView>;
		stats(label: string): Promise<StatsView>;
	};
	readonly devices: {
		list(): Promise<DeviceView[]>;
	};
	readonly scans: {
		start(label: string, onUpdate: (update: ScanUpdate) => void): Promise<void>;
		/** Follows a scan already running; the latest update arrives at once. */
		attach(label: string, onUpdate: (update: ScanUpdate) => void): Promise<void>;
		/** The latest update of every running scan. */
		running(): Promise<ScanUpdate[]>;
		pause(label: string): Promise<void>;
		resume(label: string): Promise<void>;
		cancel(label: string): Promise<void>;
		recent(offset: number, limit: number): Promise<RecentScanView[]>;
		recentCount(): Promise<number>;
		ofLocation(label: string, offset: number, limit: number): Promise<ScanBrief[]>;
		countOfLocation(label: string): Promise<number>;
		warnings(scanId: number, offset: number, limit: number): Promise<WarningView[]>;
		warningCount(scanId: number): Promise<number>;
	};
	readonly browse: {
		childCount(label: string, parent: string): Promise<number>;
		children(label: string, parent: string, offset: number, limit: number): Promise<EntryView[]>;
		/** Calls `onChange` while scans of `label` write; returns the function that stops it. */
		subscribe(label: string, onChange: (change: BrowseChange) => void): () => void;
	};
	readonly compare: {
		open(request: CompareRequest, onUpdate: (update: VerifyUpdate) => void): Promise<CompareOpened>;
		close(id: number): Promise<void>;
		/** Closes every open comparison; a window that starts again calls it for those it lost. */
		closeAll(): Promise<void>;
		verify(id: number): Promise<void>;
		cancelVerify(id: number): Promise<void>;
		summary(id: number, filter: ApiFilter): Promise<SummaryView>;
		count(id: number, filter: ApiFilter): Promise<number>;
		rows(
			id: number,
			filter: ApiFilter,
			order: RowOrder,
			offset: number,
			limit: number
		): Promise<CompareRowView[]>;
		childCount(id: number, filter: ApiFilter, parent: string): Promise<number>;
		children(
			id: number,
			filter: ApiFilter,
			parent: string,
			offset: number,
			limit: number
		): Promise<TreeNodeView[]>;
		byExtension(id: number, filter: ApiFilter): Promise<ExtensionView[]>;
		sizeHistogram(id: number, filter: ApiFilter): Promise<SizeBucketView[]>;
		duplicates(
			id: number,
			side: number,
			offset: number,
			limit: number
		): Promise<DuplicateSetView[]>;
		duplicateTotals(id: number): Promise<DuplicateTotalsView[]>;
	};
}
