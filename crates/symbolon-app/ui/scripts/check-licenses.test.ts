import { describe, expect, it } from 'vitest';
import { isAllowed } from './check-licenses';

describe('isAllowed', () => {
	it.each([
		['MIT', true],
		['Apache-2.0 OR MIT', true],
		['(MIT OR GPL-2.0-only)', true],
		['MIT AND Zlib', true],
		['MIT AND GPL-2.0-only', false],
		['GPL-2.0-only', false],
		['UNLICENSED', false],
		['(MIT OR Apache-2.0) AND BSD-3-Clause', false]
	])('%s → %s', (expression, expected) => {
		expect(isAllowed(expression)).toBe(expected);
	});
});
