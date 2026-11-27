import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { EntryView, MemorySeed } from '../api/index.ts';
import { expectNoAxeViolations } from '../../test/axe';
import { location, renderWithBackend } from '../../test/backend';
import { formatAgo } from '../format';
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
			scan: lastScan,
			latest: true
		}
	],
	warnings: { 9: [{ path: '2024/locked.jpg', kind: 'permission', message: 'permission denied' }] }
});

function detail(label = 'Photos', data = seed()) {
	const navigate = vi.fn();
	return { ...renderWithBackend(LocationDetail, { label, navigate }, data), navigate };
}

describe('LocationDetail', () => {
	it('leads back to the list of locations', async () => {
		detail();
		const trail = await screen.findByRole('navigation', { name: 'Breadcrumb' });
		expect(within(trail).getByRole('link', { name: 'Locations' }).getAttribute('href')).toBe(
			'/locations'
		);
		expect(within(trail).getByText('Photos').getAttribute('aria-current')).toBe('page');
	});

	it('says when it was last scanned, linking to that scan', async () => {
		detail();
		const last = await screen.findByRole('link', { name: formatAgo(lastScan.finishedMs) });
		expect(last.getAttribute('href')).toBe('/scans/9');
		expect(screen.getByText(/Hashed when compared/)).toBeTruthy();
	});

	it('renames and removes the location from its own page', async () => {
		const user = userEvent.setup({ pointerEventsCheck: 0 });
		const { navigate } = detail();
		await user.click(await screen.findByRole('button', { name: 'Actions for Photos' }));
		await user.click(await screen.findByRole('menuitem', { name: 'Rename…' }));
		const rename = await screen.findByRole('dialog', { name: 'Rename Photos' });
		await user.clear(within(rename).getByLabelText('New label'));
		await user.type(within(rename).getByLabelText('New label'), 'Pics');
		await user.click(within(rename).getByRole('button', { name: 'Rename' }));
		await waitFor(() => {
			expect(navigate).toHaveBeenCalledWith('/locations/Pics');
		});
	});

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
		expect(screen.getByRole('button', { name: 'Scan' }).getAttribute('aria-busy')).toBe('true');
	});

	it('browses the catalog and follows a running scan', async () => {
		const user = userEvent.setup();
		const data = seed();
		const { backend } = detail('Photos', data);
		const tree = await screen.findByRole('tree', { name: 'Files of Photos' });
		await waitFor(() => {
			expect(within(tree).getAllByRole('treeitem')).toHaveLength(2);
		});
		expect(within(tree).getByText('10 B')).toBeTruthy();
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
		const { navigate } = detail();
		await user.click(await screen.findByRole('tab', { name: 'Scans' }));
		const history = await screen.findByRole('grid', { name: 'Scans of Photos' });
		await waitFor(() => {
			expect(within(history).getByText('Complete')).toBeTruthy();
		});
		expect(within(history).getByText('Latest')).toBeTruthy();
		await user.click(within(history).getByText('Complete'));
		expect(navigate).toHaveBeenCalledWith('/scans/9');
		await user.click(screen.getByRole('tab', { name: 'Warnings 1' }));
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

describe('LocationDetail after a scan', () => {
	it('reads the history again when a scan of this location ends', async () => {
		const user = userEvent.setup();
		const data = seed();
		const { backend, scans } = detail('Photos', data);
		await user.click(await screen.findByRole('tab', { name: 'Scans' }));
		const history = await screen.findByRole('grid', { name: 'Scans of Photos' });
		await waitFor(() => {
			expect(history.getAttribute('aria-rowcount')).toBe('2');
		});
		await scans.start('Photos');
		data.recentScans = [
			{ label: 'Photos', scan: { ...lastScan, scanId: 10 }, latest: true },
			...(data.recentScans ?? [])
		];
		backend.emitScan('Photos', {
			type: 'Finished',
			label: 'Photos',
			scanId: 10,
			status: 'Completed',
			files: 1,
			bytes: 1,
			hashedFiles: 0,
			warnings: 0,
			elapsedMs: 1
		});
		await waitFor(() => {
			expect(
				screen.getByRole('grid', { name: 'Scans of Photos' }).getAttribute('aria-rowcount')
			).toBe('3');
		});
	});
});
