import type { ScanStatus, ScanUpdate } from '../api/index.ts';

/** How far a running scan is: its phase, a percentage once known, and the time left. */
export interface ScanProgress {
	label: string;
	/** 0–100 while hashing; undefined while files are listed. */
	percent?: number;
	remaining?: string;
}

/** A duration for people: "45 s", "3 min 5 s", "1 h 2 min". */
export function formatDuration(ms: number): string {
	const seconds = Math.max(0, Math.round(ms / 1000));
	if (seconds < 60) return `${String(seconds)} s`;
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) return `${String(minutes)} min ${String(seconds % 60)} s`;
	return `${String(Math.floor(minutes / 60))} h ${String(minutes % 60)} min`;
}

/** The progress `update` shows. Time left is estimated from the hashing rate only. */
export function progressOf(update: ScanUpdate): ScanProgress {
	if (update.type !== 'Progress') return { label: 'Starting' };
	if (update.phase === 'Walking') return { label: 'Listing files' };
	const total = update.toHashBytes;
	const percent = total > 0 ? Math.min(100, Math.floor((update.hashedBytes / total) * 100)) : 0;
	const left = Math.max(0, total - update.hashedBytes);
	return {
		label: 'Hashing',
		percent,
		...(update.bytesPerSec > 0 && {
			remaining: `${formatDuration((left / update.bytesPerSec) * 1000)} left`
		})
	};
}

const OUTCOMES: Record<ScanStatus, string> = {
	Running: 'Running',
	Completed: 'Complete',
	Cancelled: 'Cancelled',
	Failed: 'Failed',
	// The app stopped during the scan; what it committed is kept.
	Interrupted: 'Partial'
};

/** How a recorded scan ended, in a word. */
export function outcomeLabel(status: ScanStatus): string {
	return OUTCOMES[status];
}
