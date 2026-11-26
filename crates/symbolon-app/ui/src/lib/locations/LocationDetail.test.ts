import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { EntryView, MemorySeed } from '../api/index.ts';
import { expectNoAxeViolations } from '../../test/axe';
import { location, renderWithBackend } from '../../test/backend';
import LocationDetail from './LocationDetail.svelte';

const entry = (key: string, folder = false): EntryView => ({
	key,
	name: key.split('/').at(-1) ?? key,
	kind: folder ? 'Dir' : 'File',
	folder,
	size: 10,
	mtimeMs: null,
	hashState: 'Unhashed'
});

const lastScan = {
	scanId: 9,
	status: 'Completed' as const,
	startedMs: Date.now() - 60_000,
	finishedMs: Date.now() - 30_000,
	files: 1204,
	bytes: 3 * 1024 ** 3,
	warnings: 1,
	error: null
};

const seed = (): MemorySeed => ({
	locations: [
		location('Photos', {
			lastScan,
			stats: {
				files: 1204,
				dirs: 12,
				symlinks: 0,
				bytes: 3 * 1024 ** 3,
				verifiedFiles: 602,
				partialFiles: 0
			}
		})
	],
	entries: {
		Photos: { '': [entry('2024', true), entry('cover.jpg')], '2024': [entry('2024/a.jpg')] }
	},
	recentScans: [
		{
			label: 'Photos',
			scan: lastScan
		}
	],
	warnings: { 9: [{ path: '2024/locked.jpg', kind: 'permission', message: 'permission denied' }] }
});

function detail(label = 'Photos', data = seed()) {
	const navigate = vi.fn();
	return { ...renderWithBackend(LocationDetail, { label, navigate }, data), navigate };
}

describe('LocationDetail', () => {
	it('shows what the catalog holds', async () => {
		const { container } = detail();
		expect(await screen.findByText('1,204')).toBeTruthy();
		expect(screen.getByText('12')).toBeTruthy();
		expect(screen.getByText('3.0 GiB')).toBeTruthy();
		expect(screen.getByText('50%')).toBeTruthy();
		await expectNoAxeViolations(container);
	});

	it('scans and shows the progress', async () => {
		const user = userEvent.setup();
		const { backend } = detail();
		await user.click(await screen.findByRole('button', { name: 'Scan' }));
		expect(backend.calls.some((c) => c.method === 'scans.start')).toBe(true);
		backend.emitScan('Photos', {
			type: 'Progress',
			label: 'Photos',
			phase: 'Walking',
			files: 40,
			dirs: 1,
			bytes: 1,
			hashedFiles: 0,
			hashedBytes: 0,
			toHashFiles: 0,
			toHashBytes: 0,
			bytesPerSec: 0,
			warnings: 0,
			elapsedMs: 10
		});
		expect(await screen.findByText('Scanning Photos — 40 files')).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Scan' }).hasAttribute('disabled')).toBe(true);
	});

	it('browses the catalog and follows a running scan', async () => {
		const user = userEvent.setup();
		const data = seed();
		const { backend } = detail('Photos', data);
		const tree = await screen.findByRole('tree', { name: 'Files of Photos' });
		await waitFor(() => {
			expect(within(tree).getAllByRole('treeitem')).toHaveLength(2);
		});
		data.entries = {
			Photos: {
				...data.entries?.['Photos'],
				'': [entry('2024', true), entry('cover.jpg'), entry('new.jpg')]
			}
		};
		backend.emitBrowse('Photos', 'Changed');
		await waitFor(() => {
			expect(within(tree).getAllByRole('treeitem')).toHaveLength(3);
		});
		await user.click(within(tree).getByText('2024'));
		await user.keyboard('l');
		await waitFor(() => {
			expect(within(tree).getByText('a.jpg')).toBeTruthy();
		});
	});

	it('lists past scans and the warnings of the last one', async () => {
		const user = userEvent.setup();
		detail();
		await user.click(await screen.findByRole('tab', { name: 'Scans' }));
		const history = await screen.findByRole('grid', { name: 'Scans of Photos' });
		await waitFor(() => {
			expect(within(history).getByText('Completed')).toBeTruthy();
		});
		await user.click(screen.getByRole('tab', { name: 'Warnings' }));
		const warnings = await screen.findByRole('grid', { name: 'Warnings of the last scan' });
		await waitFor(() => {
			expect(within(warnings).getByText('permission denied')).toBeTruthy();
		});
	});

	it('says so when the location is gone', async () => {
		detail('Gone');
		expect(await screen.findByText('This location no longer exists.')).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Back to Locations' }).getAttribute('href')).toBe(
			'/locations'
		);
	});
});
