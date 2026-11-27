const UNITS = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB'] as const;

/** Formats a byte count with binary units, e.g. `1536` → `"1.5 KiB"`. */
export function formatBytes(bytes: number): string {
	if (!Number.isFinite(bytes) || bytes < 0) {
		throw new RangeError(`invalid byte count: ${String(bytes)}`);
	}
	let value = bytes;
	let unit = 0;
	while (value >= 1024 && unit < UNITS.length - 1) {
		value /= 1024;
		unit += 1;
	}
	const digits = unit === 0 || value >= 100 ? 0 : 1;
	return `${value.toFixed(digits)} ${UNITS[unit] ?? 'B'}`;
}

const relative = new Intl.RelativeTimeFormat('en-US', { numeric: 'auto' });
const STEPS: readonly [Intl.RelativeTimeFormatUnit, number][] = [
	['year', 365 * 86_400_000],
	['month', 30 * 86_400_000],
	['week', 7 * 86_400_000],
	['day', 86_400_000],
	['hour', 3_600_000],
	['minute', 60_000]
];

/** How long ago `ms` (since the epoch) was at `now`, e.g. "3 hours ago" or "yesterday". */
export function formatAgo(ms: number, now: number = Date.now()): string {
	const elapsed = Math.max(0, now - ms);
	for (const [unit, size] of STEPS) {
		if (elapsed >= size) return relative.format(-Math.floor(elapsed / size), unit);
	}
	return 'just now';
}
