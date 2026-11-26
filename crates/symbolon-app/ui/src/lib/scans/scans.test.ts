import { describe, expect, it, vi } from 'vitest';
import { createMemoryBackend, type ScanUpdate } from '../api/index.ts';
import { toasts } from '../feedback/toasts.svelte';
import { createScanState, statusText } from './scans.svelte';

const progress = (label: string, extra: Partial<Extract<ScanUpdate, { type: 'Progress' }>>) =>
	({
		type: 'Progress',
		label,
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
	}) satisfies ScanUpdate;

const finished = (label: string): ScanUpdate => ({
	type: 'Finished',
	label,
	scanId: 1,
	status: 'Completed',
	files: 3,
	bytes: 30,
	hashedFiles: 3,
	warnings: 0,
	elapsedMs: 10
});

function backendWith(...labels: string[]) {
	return createMemoryBackend({
		locations: labels.map((label) => ({
			label,
			root: `/${label}`,
			kind: 'Folder',
			status: { type: 'Connected' },
			hashPolicy: 'Lazy',
			hashAlgo: 'blake3',
			lastScan: null,
			stats: { files: 0, dirs: 0, symlinks: 0, bytes: 0, verifiedFiles: 0, partialFiles: 0 }
		}))
	});
}

describe('statusText', () => {
	it('says nothing runs, one scan with its progress, or how many run', () => {
		expect(statusText([])).toBe('No scans running');
		expect(statusText([progress('Photos', { files: 1204 })])).toBe('Scanning Photos — 1,204 files');
		expect(
			statusText([progress('Photos', { phase: 'Hashing', hashedBytes: 42, toHashBytes: 100 })])
		).toBe('Scanning Photos — 42%');
		expect(statusText([progress('A', {}), progress('B', {})])).toBe('2 scans running');
		expect(statusText([{ type: 'Started', label: 'Photos', scanId: 1 }])).toBe('Scanning Photos');
	});
});

describe('createScanState', () => {
	it('init attaches to running scans', async () => {
		const b = backendWith('Photos');
		await b.scans.start('Photos', () => undefined);
		b.emitScan('Photos', progress('Photos', { files: 5 }));
		const state = createScanState(b);
		await state.init();
		expect(state.running).toEqual([progress('Photos', { files: 5 })]);
		b.emitScan('Photos', progress('Photos', { files: 9 }));
		expect(state.running).toEqual([progress('Photos', { files: 9 })]);
	});

	it('finished scans leave running and raise a toast', async () => {
		const b = backendWith('Photos');
		const state = createScanState(b);
		const onFinished = vi.fn();
		state.onFinished(onFinished);
		await state.start('Photos');
		b.emitScan('Photos', progress('Photos', { files: 1 }));
		expect(state.running).toHaveLength(1);
		b.emitScan('Photos', finished('Photos'));
		expect(state.running).toEqual([]);
		expect(onFinished).toHaveBeenCalledWith(finished('Photos'));
		expect(toasts.items.at(-1)).toMatchObject({
			message: 'Photos: scan finished',
			tone: 'success'
		});
	});

	it('failed scans raise an error toast with the reason', async () => {
		const b = backendWith('Stick');
		const state = createScanState(b);
		await state.start('Stick');
		b.emitScan('Stick', {
			type: 'Failed',
			label: 'Stick',
			error: { kind: 'Unavailable', message: 'location root /stick is unavailable' }
		});
		expect(toasts.items.at(-1)).toMatchObject({
			message: 'Stick: scan failed. location root /stick is unavailable',
			tone: 'error'
		});
	});

	it('remembers paused scans', async () => {
		const b = backendWith('Photos');
		const state = createScanState(b);
		await state.start('Photos');
		await state.pause('Photos');
		expect(state.isPaused('Photos')).toBe(true);
		await state.resume('Photos');
		expect(state.isPaused('Photos')).toBe(false);
		expect(b.calls.map((c) => c.method)).toContain('scans.pause');
	});
});
