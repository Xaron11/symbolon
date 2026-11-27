import { describe, expect, it } from 'vitest';
import type { ApiError } from './errors';
import { createMemoryBackend } from './memory';
import type { ScanUpdate } from './types';

describe('memory backend', () => {
	it('adds, lists and refuses duplicate labels like the bridge', async () => {
		const b = createMemoryBackend();
		const request = {
			label: 'Photos',
			kind: { type: 'Folder' as const, path: '/photos' },
			hashPolicy: 'Lazy' as const,
			ignorePatterns: []
		};
		await b.locations.add(request);
		await expect(b.locations.list()).resolves.toHaveLength(1);
		const error = await b.locations.add(request).catch((e: unknown) => e);
		expect((error as ApiError).kind).toBe('AlreadyExists');
	});

	it('streams scan updates to followers and ends on Finished', async () => {
		const b = createMemoryBackend();
		await b.locations.add({
			label: 'Photos',
			kind: { type: 'Folder', path: '/p' },
			hashPolicy: 'Lazy',
			ignorePatterns: []
		});
		const seen: ScanUpdate[] = [];
		await b.scans.start('Photos', (u) => seen.push(u));
		b.emitScan('Photos', { type: 'Started', label: 'Photos', scanId: 1 });
		const late: ScanUpdate[] = [];
		await b.scans.attach('Photos', (u) => late.push(u));
		expect(late).toEqual([{ type: 'Started', label: 'Photos', scanId: 1 }]);
		await expect(b.scans.running()).resolves.toHaveLength(1);
		await expect(b.locations.remove('Photos')).rejects.toMatchObject({ kind: 'AlreadyScanning' });
		b.emitScan('Photos', {
			type: 'Finished',
			label: 'Photos',
			scanId: 1,
			status: 'Completed',
			files: 1,
			bytes: 1,
			hashedFiles: 0,
			warnings: 0,
			elapsedMs: 5
		});
		expect(seen).toHaveLength(2);
		await expect(b.scans.running()).resolves.toEqual([]);
	});

	it('closed comparisons are not found', async () => {
		const b = createMemoryBackend({ comparison: { summary: { categories: [] }, rows: [] } });
		const { id } = await b.compare.open(
			{
				sides: [{ label: 'a' }, { label: 'b' }],
				nfc: false,
				caseInsensitive: false,
				duplicates: false
			},
			() => undefined
		);
		await b.compare.close(id);
		await expect(b.compare.count(id, {})).rejects.toMatchObject({ kind: 'NotFound' });
	});
});
