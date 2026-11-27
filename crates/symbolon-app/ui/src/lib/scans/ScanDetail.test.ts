import { screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { MemorySeed, RecentScanView } from '../api/index.ts';
import { expectNoAxeViolations } from '../../test/axe';
import { location, renderWithBackend } from '../../test/backend';
import ScanDetail from './ScanDetail.svelte';

const scan = (overrides: Partial<RecentScanView['scan']> = {}, latest = true): RecentScanView => ({
	label: 'Photos',
	scan: {
		scanId: 7,
		status: 'Completed',
		startedMs: Date.UTC(2026, 10, 27, 9, 0, 0),
		finishedMs: Date.UTC(2026, 10, 27, 9, 3, 20),
		files: 1204,
		bytes: 3 * 1024 ** 3,
		warnings: 1,
		error: null,
		...overrides
	},
	latest
});

function detail(found: RecentScanView[] = [scan()], id = 7) {
	const navigate = vi.fn();
	const seed: MemorySeed = {
		locations: [location('Photos')],
		recentScans: found,
		warnings: { 7: [{ path: '2024/locked.jpg', kind: 'permission', message: 'permission denied' }] }
	};
	return { ...renderWithBackend(ScanDetail, { scanId: id, navigate }, seed), navigate };
}

describe('ScanDetail', () => {
	it('shows what one scan found and where it leads', async () => {
		const { container } = detail();
		expect(await screen.findByRole('heading', { level: 2, name: 'Photos' })).toBeTruthy();
		const trail = screen.getByRole('navigation', { name: 'Breadcrumb' });
		expect(within(trail).getByRole('link', { name: 'Scans' }).getAttribute('href')).toBe('/scans');
		expect(screen.getByText('Complete')).toBeTruthy();
		expect(screen.getByText('Latest scan')).toBeTruthy();
		expect(screen.getByText('1,204')).toBeTruthy();
		expect(screen.getByText('3.0 GiB')).toBeTruthy();
		expect(screen.getByText('3 min 20 s')).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Open Photos' }).getAttribute('href')).toBe(
			'/locations/Photos'
		);
		const warnings = await screen.findByRole('grid', { name: 'Warnings of this scan' });
		expect(await within(warnings).findByText('permission denied')).toBeTruthy();
		await expectNoAxeViolations(container);
	});

	it('says why a scan failed and that a newer one exists', async () => {
		detail([scan({ status: 'Failed', error: 'disk went away' }, false)]);
		expect(await screen.findByText('disk went away')).toBeTruthy();
		expect(screen.getByText('A newer scan of Photos exists.')).toBeTruthy();
		expect(screen.queryByText('Latest scan')).toBeNull();
	});

	it('scans the location again', async () => {
		const user = userEvent.setup();
		const { backend } = detail();
		await user.click(await screen.findByRole('button', { name: 'Scan again' }));
		expect(backend.calls.some((c) => c.method === 'scans.start')).toBe(true);
	});

	it('says in a sentence that a scan had no warnings', async () => {
		detail([scan({ warnings: 0 })]);
		expect(await screen.findByText('This scan recorded no warnings.')).toBeTruthy();
		expect(screen.queryByRole('grid')).toBeNull();
	});

	it('says so when the scan is gone', async () => {
		detail([], 99);
		expect(await screen.findByText('This scan no longer exists.')).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Back to Scans' }).getAttribute('href')).toBe('/scans');
	});
});
