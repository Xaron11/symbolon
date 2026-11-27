import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { ApiError, type RecentScanView, type ScanUpdate } from '../api/index.ts';
import { toasts } from '../feedback/toasts.svelte';
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
	},
	latest: false
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
		const fact = (name: string) => inCard.getByText(name).nextElementSibling?.textContent;
		expect(fact('Files')).toBe('1,204');
		expect(fact('Speed')).toBe('64.0 MiB/s');
		expect(fact('Elapsed')).toBe('1 min 5 s');
		expect(fact('Time left')).toBe('8 s');
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

describe('ScansView details', () => {
	it('names a card after a label with spaces and keeps focus on Pause/Resume', async () => {
		const user = userEvent.setup();
		const { backend, scans } = view({ locations: [location('My Photos')] });
		await scans.start('My Photos');
		backend.emitScan('My Photos', { ...hashing, label: 'My Photos' });
		const card = await screen.findByRole('article', { name: 'My Photos' });
		const pause = within(card).getByRole('button', { name: 'Pause' });
		pause.focus();
		await user.keyboard('{Enter}');
		await waitFor(() => {
			expect(within(card).getByRole('button', { name: 'Resume' })).toBe(document.activeElement);
		});
		expect(within(card).getByText('Paused')).toBeTruthy();
		expect(within(card).queryByText('Time left')).toBeNull();
		await user.keyboard('{Enter}');
		expect(backend.calls.map((c) => c.method)).toContain('scans.resume');
	});

	it('shows a failure to read the history instead of claiming there is none', async () => {
		renderWithBackend(
			ScansView,
			{ navigate: vi.fn() },
			{ locations: [location('Photos')] },
			(b) => {
				b.scans.recentCount = () => Promise.reject(new ApiError('Io', 'database is locked'));
			}
		);
		expect(await screen.findByText('database is locked')).toBeTruthy();
		expect(screen.queryByText('No scans yet')).toBeNull();
	});

	it('gives the page section headings', async () => {
		const { backend, scans } = view({ recentScans: [recent('Photos', 'Completed')] });
		await scans.start('Photos');
		backend.emitScan('Photos', hashing);
		expect(await screen.findByRole('heading', { level: 2, name: 'Running' })).toBeTruthy();
		expect(screen.getByRole('heading', { level: 2, name: 'Recent scans' })).toBeTruthy();
	});

	it('opens a recent scan with one click or Enter', async () => {
		const user = userEvent.setup();
		const { navigate } = view({ recentScans: [recent('My Photos', 'Completed')] });
		const table = await screen.findByRole('grid', { name: 'Recent scans' });
		await waitFor(() => {
			expect(within(table).getByText('My Photos')).toBeTruthy();
		});
		await user.click(within(table).getByText('My Photos'));
		expect(navigate).toHaveBeenLastCalledWith('/scans/1');
		navigate.mockClear();
		table.focus();
		await user.keyboard('{Enter}');
		expect(navigate).toHaveBeenLastCalledWith('/scans/1');
	});

	it('marks each location latest scan and says how long scans took', async () => {
		const latest = { ...recent('Photos', 'Completed'), latest: true };
		view({ recentScans: [latest, recent('Photos', 'Completed')] });
		const table = await screen.findByRole('grid', { name: 'Recent scans' });
		await waitFor(() => {
			expect(within(table).getAllByText('Latest')).toHaveLength(1);
		});
		expect(within(table).getAllByText('3 min 20 s')).toHaveLength(2);
	});

	it('sums up the history above it', async () => {
		view({
			locations: [location('Photos'), location('Stick')],
			recentScans: [recent('Photos', 'Completed'), recent('Stick', 'Completed')]
		});
		expect(await screen.findByText('2 scans of 2 locations')).toBeTruthy();
	});

	it('starts a scan from the screen', async () => {
		const user = userEvent.setup({ pointerEventsCheck: 0 });
		const { backend } = view({ recentScans: [recent('Photos', 'Completed')] });
		await user.click(await screen.findByRole('button', { name: 'Scan location' }));
		await user.click(await screen.findByRole('menuitem', { name: 'Photos' }));
		await waitFor(() => {
			expect(backend.calls.some((c) => c.method === 'scans.start')).toBe(true);
		});
	});

	it('shows the history once the first scan ends', async () => {
		const { backend, scans } = view();
		expect(await screen.findByText('No scans yet')).toBeTruthy();
		await scans.start('Photos');
		backend.seed.recentScans = [recent('Photos', 'Completed')];
		backend.emitScan('Photos', {
			type: 'Finished',
			label: 'Photos',
			scanId: 1,
			status: 'Completed',
			files: 1,
			bytes: 1,
			hashedFiles: 0,
			warnings: 0,
			elapsedMs: 1
		});
		expect(await screen.findByRole('grid', { name: 'Recent scans' })).toBeTruthy();
	});

	it('says why a control failed', async () => {
		const user = userEvent.setup();
		const { backend, scans } = view();
		await scans.start('Photos');
		backend.emitScan('Photos', hashing);
		backend.scans.cancel = () =>
			Promise.reject(new ApiError('NotFound', 'no scan of "Photos" is running'));
		await user.click(await screen.findByRole('button', { name: 'Cancel' }));
		await waitFor(() => {
			expect(toasts.items.at(-1)?.message).toBe('no scan of "Photos" is running');
		});
	});
});
