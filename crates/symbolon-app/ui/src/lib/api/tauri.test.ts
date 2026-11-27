import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import type { Channel } from '@tauri-apps/api/core';
import { afterEach, describe, expect, it } from 'vitest';
import { ApiError } from './errors';
import { createTauriBackend } from './tauri';
import type { ScanUpdate } from './types';

afterEach(() => {
	clearMocks();
});

describe('tauriBackend', () => {
	it('locations.list calls locations_list', async () => {
		const seen: string[] = [];
		mockIPC((cmd) => {
			seen.push(cmd);
			return [];
		});
		await expect(createTauriBackend().locations.list()).resolves.toEqual([]);
		expect(seen).toEqual(['locations_list']);
	});

	it('errors become ApiError with kind', async () => {
		mockIPC(() => {
			// eslint-disable-next-line @typescript-eslint/only-throw-error -- commands reject with { kind, message }
			throw { kind: 'AlreadyExists', message: 'a location labelled "Photos" already exists' };
		});
		const error = await createTauriBackend()
			.locations.add({
				label: 'Photos',
				kind: { type: 'Folder', path: '/p' },
				hashPolicy: 'Lazy',
				ignorePatterns: []
			})
			.catch((e: unknown) => e);
		expect(error).toBeInstanceOf(ApiError);
		expect((error as ApiError).kind).toBe('AlreadyExists');
	});

	it('scans.start passes a channel and delivers updates', async () => {
		let channel: Channel<ScanUpdate> | undefined;
		mockIPC((cmd, payload) => {
			expect(cmd).toBe('scan_start');
			const args = payload as { label: string; onUpdate: Channel<ScanUpdate> };
			expect(args.label).toBe('Photos');
			channel = args.onUpdate;
			return null;
		});
		const received: ScanUpdate[] = [];
		await createTauriBackend().scans.start('Photos', (update) => received.push(update));
		channel?.onmessage({ type: 'Started', label: 'Photos', scanId: 7 });
		expect(received).toEqual([{ type: 'Started', label: 'Photos', scanId: 7 }]);
	});

	it('compare.rows sends filter, order, offset, limit', async () => {
		let args: unknown;
		mockIPC((cmd, payload) => {
			expect(cmd).toBe('compare_rows');
			args = payload;
			return [];
		});
		await createTauriBackend().compare.rows(
			3,
			{ categories: ['different'] },
			{ by: 'Size', desc: true },
			200,
			100
		);
		expect(args).toEqual({
			id: 3,
			filter: { categories: ['different'] },
			order: { by: 'Size', desc: true },
			offset: 200,
			limit: 100
		});
	});

	it('locations.followMove sends null for no choice', async () => {
		let args: unknown;
		mockIPC((_cmd, payload) => {
			args = payload;
			return {};
		});
		await createTauriBackend().locations.followMove('Stick');
		expect(args).toEqual({ label: 'Stick', choice: null });
	});
});

describe('tauriBackend releases what it opened', () => {
	it('browse.subscribe unsubscribes by id when stopped', async () => {
		const seen: [string, unknown][] = [];
		mockIPC((cmd, payload) => {
			seen.push([cmd, payload]);
			return cmd === 'browse_subscribe' ? 41 : null;
		});
		const stop = createTauriBackend().browse.subscribe('Photos', () => undefined);
		await new Promise((resolve) => setTimeout(resolve, 0));
		stop();
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(seen.map(([cmd]) => cmd)).toEqual(['browse_subscribe', 'browse_unsubscribe']);
		expect(seen[1]?.[1]).toEqual({ id: 41 });
	});

	it('a subscription stopped before it is registered is released once it is', async () => {
		const seen: string[] = [];
		mockIPC((cmd) => {
			seen.push(cmd);
			return cmd === 'browse_subscribe' ? 7 : null;
		});
		createTauriBackend().browse.subscribe('Photos', () => undefined)();
		await new Promise((resolve) => setTimeout(resolve, 0));
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(seen).toEqual(['browse_subscribe', 'browse_unsubscribe']);
	});

	it('compare.closeAll calls compare_close_all', async () => {
		const seen: string[] = [];
		mockIPC((cmd) => {
			seen.push(cmd);
			return null;
		});
		await createTauriBackend().compare.closeAll();
		expect(seen).toEqual(['compare_close_all']);
	});
});

describe('tauriBackend scans', () => {
	it('get reads one scan, and undefined for none', async () => {
		let seen: unknown;
		mockIPC((cmd, payload) => {
			seen = [cmd, payload];
			return null;
		});
		await expect(createTauriBackend().scans.get(4)).resolves.toBeUndefined();
		expect(seen).toEqual(['scan_get', { scanId: 4 }]);
	});
});

describe('tauriBackend system', () => {
	it('pickFolder opens a directory picker and returns its path', async () => {
		let seen: [string, unknown] | undefined;
		mockIPC((cmd, payload) => {
			seen = [cmd, payload];
			return '/home/me/Photos';
		});
		await expect(createTauriBackend().system.pickFolder()).resolves.toBe('/home/me/Photos');
		expect(seen?.[0]).toBe('plugin:dialog|open');
		expect(seen?.[1]).toMatchObject({ options: { directory: true, multiple: false } });
	});

	it('pickFolder returns undefined when cancelled', async () => {
		mockIPC(() => null);
		await expect(createTauriBackend().system.pickFolder()).resolves.toBeUndefined();
	});
});
