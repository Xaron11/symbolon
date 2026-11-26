import { getContext, setContext } from 'svelte';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import type { Backend, ScanUpdate } from '../api/index.ts';
import { toasts } from '../feedback/toasts.svelte';

const count = new Intl.NumberFormat('en-US');

/** The status bar's words for the running scans. */
export function statusText(running: readonly ScanUpdate[]): string {
	if (running.length === 0) return 'No scans running';
	if (running.length > 1) return `${String(running.length)} scans running`;
	const [scan] = running;
	if (scan?.type !== 'Progress') return `Scanning ${scan?.label ?? ''}`;
	if (scan.phase === 'Hashing' && scan.toHashBytes > 0) {
		const percent = Math.min(100, Math.floor((scan.hashedBytes / scan.toHashBytes) * 100));
		return `Scanning ${scan.label} — ${String(percent)}%`;
	}
	return `Scanning ${scan.label} — ${count.format(scan.files)} files`;
}

/** Scans running in the backend, as the whole app sees them. */
export interface ScanState {
	/** The latest update of each running scan. */
	readonly running: readonly ScanUpdate[];
	isPaused(label: string): boolean;
	start(label: string): Promise<void>;
	pause(label: string): Promise<void>;
	resume(label: string): Promise<void>;
	cancel(label: string): Promise<void>;
	/** Follows scans already running in the backend (after a reload). */
	init(): Promise<void>;
	/** Calls `listener` with the final update of every scan that ends. */
	onFinished(listener: (update: ScanUpdate) => void): () => void;
}

/** Tracks scans started through `backend`, telling people when one ends. */
export function createScanState(backend: Backend): ScanState {
	const latest = new SvelteMap<string, ScanUpdate>();
	const paused = new SvelteSet<string>();
	const finished = new SvelteSet<(update: ScanUpdate) => void>();

	const receive = (update: ScanUpdate) => {
		if (update.type === 'Finished' || update.type === 'Failed') {
			latest.delete(update.label);
			paused.delete(update.label);
			if (update.type === 'Failed') {
				toasts.show(`${update.label}: scan failed. ${update.error.message}`, 'error');
			} else if (update.status === 'Completed') {
				toasts.show(`${update.label}: scan finished`, 'success');
			} else {
				toasts.show(`${update.label}: scan ${update.status.toLowerCase()}`, 'warning');
			}
			for (const listener of [...finished]) listener(update);
			return;
		}
		latest.set(update.label, update);
	};

	return {
		get running() {
			return [...latest.values()];
		},
		isPaused: (label) => paused.has(label),
		async start(label) {
			await backend.scans.start(label, receive);
			if (!latest.has(label)) latest.set(label, { type: 'Started', label, scanId: 0 });
		},
		async pause(label) {
			await backend.scans.pause(label);
			paused.add(label);
		},
		async resume(label) {
			await backend.scans.resume(label);
			paused.delete(label);
		},
		cancel: (label) => backend.scans.cancel(label),
		async init() {
			for (const update of await backend.scans.running()) {
				latest.set(update.label, update);
				await backend.scans.attach(update.label, receive);
			}
		},
		onFinished(listener) {
			finished.add(listener);
			return () => finished.delete(listener);
		}
	};
}

const KEY = Symbol('sy-scans');

/** Makes `state` available to the components below the caller. */
export function provideScans(state: ScanState): void {
	setContext(KEY, state);
}

/** The scan state provided by an ancestor; throws when there is none. */
export function scans(): ScanState {
	const found = getContext<ScanState | undefined>(KEY);
	if (!found) throw new Error('no scan state provided; call provideScans in a parent');
	return found;
}
