import {
	createChangeHub,
	toApiError,
	type ApiError,
	type ApiFilter,
	type Backend,
	type ChangeHub,
	type CompareRequest,
	type SideRequest,
	type SummaryView,
	type VerifyUpdate
} from '../api/index.ts';

/** Background verification of an open comparison. */
export interface VerifyState {
	state: 'running' | 'done' | 'cancelled' | 'failed';
	/** The latest hashing progress while running. */
	progress?: Extract<VerifyUpdate, { type: 'Progress' }>;
	error?: string;
}

/** One open comparison, as a screen sees it. */
export interface CompareSession {
	/** Undefined until the comparison is open. */
	readonly id: number | undefined;
	readonly sides: readonly SideRequest[];
	readonly summary: SummaryView | undefined;
	readonly verify: VerifyState;
	/** Why it could not be opened. */
	readonly error: ApiError | undefined;
	/** Which rows the drill-down and charts show. */
	filter: ApiFilter;
	/** Notified whenever verification refreshed the results. */
	readonly changes: ChangeHub;
	open(): Promise<void>;
	verifyAgain(): Promise<void>;
	cancelVerify(): Promise<void>;
	close(): void;
}

/** A comparison of `request` over `backend`; call `open`, and `close` when leaving. */
export function createCompareSession(backend: Backend, request: CompareRequest): CompareSession {
	let id = $state<number | undefined>(undefined);
	let sides = $state.raw<readonly SideRequest[]>(
		request.sides.map((s) => ({ label: s.label, subpath: s.subpath ?? null }))
	);
	let summary = $state.raw<SummaryView | undefined>(undefined);
	let verify = $state.raw<VerifyState>({ state: 'running' });
	let error = $state.raw<ApiError | undefined>(undefined);
	let filter = $state.raw<ApiFilter>({});
	let closed = false;
	const changes = createChangeHub();

	const receive = (update: VerifyUpdate) => {
		switch (update.type) {
			case 'Progress':
				verify = { state: 'running', progress: update };
				break;
			case 'ResultsChanged':
				summary = update.summary;
				changes.notify();
				break;
			case 'Done':
				verify = { state: 'done' };
				break;
			case 'Cancelled':
				verify = { state: 'cancelled' };
				break;
			case 'Failed':
				verify = { state: 'failed', error: update.error.message };
				break;
		}
	};

	return {
		get id() {
			return id;
		},
		get sides() {
			return sides;
		},
		get summary() {
			return summary;
		},
		get verify() {
			return verify;
		},
		get error() {
			return error;
		},
		get filter() {
			return filter;
		},
		set filter(next) {
			filter = next;
		},
		changes,
		async open() {
			try {
				const opened = await backend.compare.open(request, receive);
				if (closed) {
					void backend.compare.close(opened.id).catch(() => undefined);
					return;
				}
				id = opened.id;
				sides = opened.sides;
				summary = opened.summary;
			} catch (e) {
				error = toApiError(e);
			}
		},
		async verifyAgain() {
			if (id === undefined) return;
			verify = { state: 'running' };
			await backend.compare.verify(id);
		},
		async cancelVerify() {
			if (id !== undefined) await backend.compare.cancelVerify(id);
		},
		close() {
			closed = true;
			if (id !== undefined) void backend.compare.close(id).catch(() => undefined);
		}
	};
}
