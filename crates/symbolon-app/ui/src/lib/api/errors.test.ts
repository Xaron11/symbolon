import { describe, expect, it } from 'vitest';
import { ApiError, toApiError } from './errors';

describe('toApiError', () => {
	it('keeps the kind and message of a command error', () => {
		const error = toApiError({ kind: 'NotFound', message: 'no location labelled "x"' });
		expect(error).toBeInstanceOf(ApiError);
		expect(error.kind).toBe('NotFound');
		expect(error.message).toBe('no location labelled "x"');
	});

	it('turns anything else into an internal error', () => {
		expect(toApiError(new Error('boom')).kind).toBe('Internal');
		expect(toApiError('command not found').message).toBe('command not found');
	});

	it('returns an ApiError unchanged', () => {
		const error = new ApiError('Moved', 'moved');
		expect(toApiError(error)).toBe(error);
	});
});
