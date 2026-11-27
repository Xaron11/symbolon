import { describe, expect, it } from 'vitest';
import type { ScanUpdate } from '../api/index.ts';
import { formatDuration, outcomeLabel, progressOf } from './progress';

const progress = (extra: Partial<Extract<ScanUpdate, { type: 'Progress' }>>): ScanUpdate => ({
	type: 'Progress',
	label: 'Photos',
	phase: 'Walking',
	files: 0,
	dirs: 0,
	bytes: 0,
	hashedFiles: 0,
	hashedBytes: 0,
	toHashFiles: 0,
	toHashBytes: 0,
	bytesPerSec: 0,
	warnings: 0,
	elapsedMs: 0,
	...extra
});

describe('progressOf', () => {
	it('has no percentage while listing files', () => {
		expect(progressOf(progress({ files: 1204 }))).toEqual({ label: 'Listing files' });
	});

	it('gives a percentage and the time left while hashing', () => {
		expect(
			progressOf(progress({ phase: 'Hashing', hashedBytes: 42, toHashBytes: 100, bytesPerSec: 29 }))
		).toEqual({ label: 'Hashing', percent: 42, remaining: '2 s' });
	});

	it('leaves the time out when the rate is unknown', () => {
		expect(progressOf(progress({ phase: 'Hashing', toHashBytes: 100 }))).toEqual({
			label: 'Hashing',
			percent: 0
		});
	});

	it('says a scan is starting before its first progress', () => {
		expect(progressOf({ type: 'Started', label: 'Photos', scanId: 1 })).toEqual({
			label: 'Starting'
		});
	});
});

describe('formatDuration', () => {
	it('uses seconds, minutes and hours', () => {
		expect(formatDuration(45_000)).toBe('45 s');
		expect(formatDuration(185_000)).toBe('3 min 5 s');
		expect(formatDuration(3_720_000)).toBe('1 h 2 min');
	});
});

describe('outcomeLabel', () => {
	it('names how a scan ended', () => {
		expect(outcomeLabel('Completed')).toBe('Complete');
		expect(outcomeLabel('Interrupted')).toBe('Partial');
		expect(outcomeLabel('Cancelled')).toBe('Cancelled');
	});
});

describe('formatDuration under a second', () => {
	it('says less than a second instead of zero', () => {
		expect(formatDuration(400)).toBe('< 1 s');
		expect(formatDuration(0)).toBe('< 1 s');
		expect(formatDuration(1000)).toBe('1 s');
	});
});
