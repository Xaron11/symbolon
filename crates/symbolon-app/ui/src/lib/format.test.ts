import { describe, expect, it } from 'vitest';
import { byteStep, formatAgo, formatBytes } from './format';

describe('formatBytes', () => {
	it.each([
		[0, '0 B'],
		[1023, '1023 B'],
		[1024, '1.0 KiB'],
		[1536, '1.5 KiB'],
		[100 * 1024 * 1024, '100 MiB'],
		[2 ** 60, '1024 PiB']
	])('%d → %s', (bytes, expected) => {
		expect(formatBytes(bytes)).toBe(expected);
	});

	it('rejects negative and non-finite input', () => {
		expect(() => formatBytes(-1)).toThrow(RangeError);
		expect(() => formatBytes(Number.NaN)).toThrow(RangeError);
	});
});

describe('formatAgo', () => {
	const now = Date.UTC(2026, 10, 26, 12, 0, 0);
	it('says how long ago, in the largest whole unit', () => {
		expect(formatAgo(now - 20_000, now)).toBe('just now');
		expect(formatAgo(now - 5 * 60_000, now)).toBe('5 minutes ago');
		expect(formatAgo(now - 3 * 3_600_000, now)).toBe('3 hours ago');
		expect(formatAgo(now - 86_400_000, now)).toBe('yesterday');
		expect(formatAgo(now - 40 * 86_400_000, now)).toBe('last month');
	});
});

describe('byteStep', () => {
	it('steps by 1, 2 or 5 of a binary unit', () => {
		const MiB = 1024 ** 2;
		expect(byteStep(334 * MiB)).toBe(100 * MiB);
		expect(byteStep(40 * MiB)).toBe(10 * MiB);
		expect(byteStep(5000)).toBe(1024);
		expect(byteStep(3 * 1024 ** 3)).toBe(1024 ** 3);
	});

	it('gives a step for empty charts', () => {
		expect(byteStep(0)).toBe(1);
	});
});
