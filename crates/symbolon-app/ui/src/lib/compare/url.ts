import { ApiError, type CompareRequest, type SideRequest } from '../api/index.ts';

// A comparison lives in its URL, so a reload or a shared link opens it again:
// `sides=Photos,Backup:2024&nfc=1&ci=1&fast=2000&dupes=1`. Each label and subfolder is
// percent-encoded first, so commas and colons in them survive.

function sideSpec(side: { label: string; subpath?: string | null }): string {
	const label = encodeURIComponent(side.label);
	return side.subpath ? `${label}:${encodeURIComponent(side.subpath)}` : label;
}

/** The query that opens `request`. */
export function toQuery(request: CompareRequest): URLSearchParams {
	const query = new URLSearchParams({ sides: request.sides.map(sideSpec).join(',') });
	if (request.nfc) query.set('nfc', '1');
	if (request.caseInsensitive) query.set('ci', '1');
	if (request.fastMs !== undefined && request.fastMs !== null) {
		query.set('fast', String(request.fastMs));
	}
	if (request.duplicates) query.set('dupes', '1');
	return query;
}

/** The comparison `query` names, or an `InvalidInput` error saying what is missing. */
export function fromQuery(query: Pick<URLSearchParams, 'get' | 'has'>): CompareRequest | ApiError {
	const sides: SideRequest[] = (query.get('sides') ?? '')
		.split(',')
		.filter((spec) => spec !== '')
		.map((spec) => {
			const colon = spec.indexOf(':');
			const label = decodeURIComponent(colon < 0 ? spec : spec.slice(0, colon));
			return colon < 0
				? { label, subpath: null }
				: { label, subpath: decodeURIComponent(spec.slice(colon + 1)) };
		});
	const duplicates = query.get('dupes') === '1';
	if (sides.length < (duplicates ? 1 : 2)) {
		return new ApiError(
			'InvalidInput',
			'Choose at least two locations, or one with Find duplicates on.'
		);
	}
	const fast = Number(query.get('fast'));
	return {
		sides: sides.map((side) => (side.subpath === null ? { label: side.label } : side)),
		nfc: query.get('nfc') === '1',
		caseInsensitive: query.get('ci') === '1',
		...(query.has('fast') && Number.isFinite(fast) && { fastMs: fast }),
		duplicates
	};
}
