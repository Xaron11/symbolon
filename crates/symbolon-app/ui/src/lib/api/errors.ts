import type { ErrorKind } from './types';

/** A failed backend call: `kind` for screens to react to, the message for people. */
export class ApiError extends Error {
	/** What went wrong, for screens to react to. */
	readonly kind: ErrorKind;

	/** An error of `kind`, described by `message`. */
	constructor(kind: ErrorKind, message: string) {
		super(message);
		this.name = 'ApiError';
		this.kind = kind;
	}
}

function isErrorShape(value: unknown): value is { kind: ErrorKind; message: string } {
	return (
		typeof value === 'object' &&
		value !== null &&
		typeof (value as { kind?: unknown }).kind === 'string' &&
		typeof (value as { message?: unknown }).message === 'string'
	);
}

/**
 * The {@link ApiError} a rejected call carries. Commands reject with `{ kind, message }`; anything
 * else (a framework failure, a programming error) becomes kind `Internal`.
 */
export function toApiError(error: unknown): ApiError {
	if (error instanceof ApiError) return error;
	if (isErrorShape(error)) return new ApiError(error.kind, error.message);
	return new ApiError('Internal', error instanceof Error ? error.message : String(error));
}
