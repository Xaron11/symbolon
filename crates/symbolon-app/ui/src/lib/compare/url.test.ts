import { describe, expect, it } from 'vitest';
import { ApiError, type CompareRequest } from '../api/index.ts';
import { fromQuery, toQuery } from './url';

const request: CompareRequest = {
	sides: [{ label: 'Photos, old' }, { label: 'Back:up', subpath: '2024/Jan' }],
	nfc: true,
	caseInsensitive: false,
	fastMs: 2000,
	duplicates: true
};

describe('comparison URLs', () => {
	it('round-trip labels with commas and colons', () => {
		const query = toQuery(request);
		expect(query.get('nfc')).toBe('1');
		expect(query.has('ci')).toBe(false);
		expect(query.get('fast')).toBe('2000');
		expect(fromQuery(new URLSearchParams(query.toString()))).toEqual(request);
	});

	it('leaves options out when they are off', () => {
		const plain = toQuery({
			sides: [{ label: 'a' }, { label: 'b' }],
			nfc: false,
			caseInsensitive: false,
			duplicates: false
		});
		expect(plain.toString()).toBe('sides=a%2Cb');
		expect(fromQuery(plain)).toEqual({
			sides: [{ label: 'a' }, { label: 'b' }],
			nfc: false,
			caseInsensitive: false,
			duplicates: false
		});
	});

	it('missing sides is InvalidInput', () => {
		const result = fromQuery(new URLSearchParams('nfc=1'));
		expect(result).toBeInstanceOf(ApiError);
		expect((result as ApiError).kind).toBe('InvalidInput');
		expect(fromQuery(new URLSearchParams('sides=only'))).toBeInstanceOf(ApiError);
		expect(fromQuery(new URLSearchParams('sides=only&dupes=1'))).not.toBeInstanceOf(ApiError);
	});
});
