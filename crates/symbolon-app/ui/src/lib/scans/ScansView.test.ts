import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { RecentScanView, ScanUpdate } from '../api/index.ts';
import { expectNoAxeViolations } from '../../test/axe';
import { location, renderWithBackend } from '../../test/backend';
import ScansView from './ScansView.svelte';

const hashing: ScanUpdate = {
	type: 'Progress',
	label: 'Photos',
	phase: 'Hashing',
	files: 1204,
	dirs: 10,
	bytes: 2 * 1024 ** 3,
	hashedFiles: 300,
	hashedBytes: 512 * 1024 ** 2,
	toHashFiles: 1204,
	toHashBytes: 1024 ** 3,
	bytesPerSec: 64 * 1024 ** 2,
	warnings: 0,
	elapsedMs: 65_000
};

const recent = (label: string, status: RecentScanView['scan']['status']): RecentScanView => ({
	label,
	scan: {
		scanId: 1,
		status,
		startedMs: Date.now() - 7_200_000,
		finishedMs: Date.now() - 7_000_000,
		files: 12,
		bytes: 2048,
		warnings: 0,
		error: null
	}
});

function view(seed = {}) {
	const navigate = vi.fn();
	const rendered = renderWithBackend(
		ScansView,
		{ navigate },
		{ locations: [location('Photos')], ...seed }
	);
	return { ...rendered, navigate };
}

describe('ScansView', () => {
	it('invites a first scan when there is nothing to show', async () => {
		const { container } = view();
		expect(await screen.findByText('No scans yet')).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Go to Locations' }).getAttribute('href')).toBe(
			'/locations'
		);
		await expectNoAxeViolations(container);
	});

	it('shows a running scan with its progress and controls', async () => {
		const user = userEvent.setup();
		const { backend, scans, container } = view();
		await scans.start('Photos');
		backend.emitScan('Photos', hashing);
		const inCard = within(await screen.findByRole('article', { name: 'Photos' }));
		expect(screen.getByRole('link', { name: 'Photos' }).getAttribute('href')).toBe(
			'/locations/Photos'
		);
		expect(inCard.getByText('Hashing')).toBeTruthy();
		expect(
			inCard.getByRole('progressbar', { name: 'Scanning Photos' }).getAttribute('aria-valuenow')
		).toBe('50');
		expect(inCard.getByText('1,204 files')).toBeTruthy();
		expect(inCard.getByText('64.0 MiB/s')).toBeTruthy();
		expect(inCard.getByText('1 min 5 s')).toBeTruthy();
		expect(inCard.getByText('8 s left')).toBeTruthy();
		await user.click(inCard.getByRole('button', { name: 'Pause' }));
		expect(await inCard.findByRole('button', { name: 'Resume' })).toBeTruthy();
		await user.click(inCard.getByRole('button', { name: 'Cancel' }));
		expect(backend.calls.map((c) => c.method)).toEqual(
			expect.arrayContaining(['scans.pause', 'scans.cancel'])
		);
		await expectNoAxeViolations(container);
	});

	it('shows an indeterminate bar while files are listed', async () => {
		const { backend, scans } = view();
		await scans.start('Photos');
		backend.emitScan('Photos', { ...hashing, phase: 'Walking' });
		const bar = await screen.findByRole('progressbar', { name: 'Scanning Photos' });
		expect(bar.hasAttribute('aria-valuenow')).toBe(false);
		expect(screen.getByText('Listing files')).toBeTruthy();
	});

	it('lists recent scans with their outcome, and refreshes when one ends', async () => {
		const { backend, scans } = view({
			recentScans: [recent('Photos', 'Completed'), recent('Stick', 'Interrupted')]
		});
		const table = await screen.findByRole('grid', { name: 'Recent scans' });
		await waitFor(() => {
			expect(within(table).getByText('Partial')).toBeTruthy();
		});
		expect(within(table).getByText('Complete')).toBeTruthy();
		await scans.start('Photos');
		backend.seed.recentScans = [recent('Photos', 'Cancelled'), ...(backend.seed.recentScans ?? [])];
		backend.emitScan('Photos', {
			type: 'Finished',
			label: 'Photos',
			scanId: 2,
			status: 'Cancelled',
			files: 1,
			bytes: 1,
			hashedFiles: 0,
			warnings: 0,
			elapsedMs: 5
		});
		await waitFor(() => {
			expect(
				within(screen.getByRole('grid', { name: 'Recent scans' })).getByText('Cancelled')
			).toBeTruthy();
		});
		expect(screen.queryByRole('article')).toBeNull();
	});
});
