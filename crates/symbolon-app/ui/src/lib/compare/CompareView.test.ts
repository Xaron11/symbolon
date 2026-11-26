import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { MemorySeed, SummaryView } from '../api/index.ts';
import { expectNoAxeViolations } from '../../test/axe';
import { location, renderWithBackend } from '../../test/backend';
import CompareView from './CompareView.svelte';

/** The category tile named `name`. */
const tile = async (name: string) =>
	within(await screen.findByRole('group', { name: 'Categories' })).getByRole('button', {
		name: new RegExp(name)
	});
const noTile = (name: string) =>
	within(screen.getByRole('group', { name: 'Categories' })).queryByRole('button', {
		name: new RegExp(name)
	});

const summary = (unverified: number): SummaryView => ({
	categories: [
		{ category: 'identical', rows: 1204, entries: 2408, bytes: 2 * 1024 ** 3, assumedRows: 0 },
		{ category: 'different', rows: 12, entries: 24, bytes: 1024 ** 2, assumedRows: 0 },
		...(unverified > 0
			? [
					{
						category: 'unverified' as const,
						rows: unverified,
						entries: 2 * unverified,
						bytes: 4096,
						assumedRows: 0
					}
				]
			: [])
	],
	onlyIn: []
});

const seed = (): MemorySeed => ({
	locations: [location('Photos'), location('Backup')],
	comparison: {
		summary: summary(40),
		rows: [],
		extensions: [
			{ extension: 'jpg', rows: 1000, bytes: 1024 ** 3 },
			{ extension: 'mov', rows: 20, bytes: 512 * 1024 ** 2 },
			{ extension: null, rows: 5, bytes: 1024 }
		],
		sizes: [
			{ maxExclusive: 1024, rows: 3, bytes: 300 },
			{ maxExclusive: 1024 ** 2, rows: 100, bytes: 1024 ** 2 },
			{ maxExclusive: 10 * 1024 ** 2, rows: 1000, bytes: 1024 ** 3 },
			{ maxExclusive: 100 * 1024 ** 2, rows: 20, bytes: 1024 ** 3 },
			{ maxExclusive: 1024 ** 3, rows: 0, bytes: 0 },
			{ maxExclusive: null, rows: 0, bytes: 0 }
		]
	}
});

function compare(query = 'sides=Photos%2CBackup', data = seed()) {
	return renderWithBackend(CompareView, { query: new URLSearchParams(query) }, data);
}

describe('CompareView', () => {
	it('opens the comparison and shows totals by category', async () => {
		const { backend, container } = compare();
		expect(await screen.findByRole('heading', { name: 'Photos and Backup' })).toBeTruthy();
		const identical = await tile('Identical');
		expect(within(identical).getByText('1,204')).toBeTruthy();
		expect(within(identical).getByText('2.0 GiB')).toBeTruthy();
		expect(await tile('Unverified')).toBeTruthy();
		expect(backend.calls.find((c) => c.method === 'compare.open')?.args[0]).toMatchObject({
			sides: [{ label: 'Photos' }, { label: 'Backup' }]
		});
		await expectNoAxeViolations(container);
	});

	it('follows verification and can stop and restart it', async () => {
		const user = userEvent.setup();
		const { backend } = compare();
		await tile('Identical');
		backend.emitVerify(1, {
			type: 'Progress',
			label: 'Backup',
			round: 1,
			files: 400,
			bytes: 38,
			totalFiles: 1204,
			totalBytes: 100
		});
		expect(await screen.findByText('Verifying 1,204 files — 38%')).toBeTruthy();
		await user.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(backend.calls.some((c) => c.method === 'compare.cancelVerify')).toBe(true);
		backend.emitVerify(1, { type: 'Cancelled' });
		expect(await screen.findByText('Verification cancelled')).toBeTruthy();
		await user.click(screen.getByRole('button', { name: 'Verify' }));
		expect(backend.calls.some((c) => c.method === 'compare.verify')).toBe(true);
		backend.emitVerify(1, { type: 'ResultsChanged', summary: summary(0) });
		backend.emitVerify(1, { type: 'Done' });
		expect(await screen.findByText('All verified')).toBeTruthy();
		await waitFor(() => {
			expect(noTile('Unverified')).toBeNull();
		});
	});

	it('filters by a category when its tile is chosen', async () => {
		const user = userEvent.setup();
		compare();
		const different = await tile('Different');
		await user.click(different);
		expect(different.getAttribute('aria-pressed')).toBe('true');
		await user.click(different);
		expect(different.getAttribute('aria-pressed')).toBe('false');
	});

	it('names the sides of only-in rows and of moves', async () => {
		const user = userEvent.setup();
		const data = seed();
		if (data.comparison)
			data.comparison.summary = {
				categories: [
					...summary(0).categories,
					{ category: 'only_in', rows: 5, entries: 5, bytes: 500, assumedRows: 0 },
					{ category: 'moved', rows: 2, entries: 4, bytes: 64, assumedRows: 0 }
				],
				onlyIn: [
					{ sides: [0], rows: 3, entries: 3, bytes: 300, assumedRows: 0 },
					{ sides: [1], rows: 2, entries: 2, bytes: 200, assumedRows: 0 }
				]
			};
		const { backend } = compare('sides=Photos%2CBackup', data);
		const photos = await tile('Only in Photos');
		expect(within(photos).getByText('3')).toBeTruthy();
		const backup = await tile('Only in Backup');
		expect(within(backup).getByText('2')).toBeTruthy();
		expect(within(await tile('Moved')).getByText(/Photos → Backup/)).toBeTruthy();

		await user.click(backup);
		expect(backup.getAttribute('aria-pressed')).toBe('true');
		expect(photos.getAttribute('aria-pressed')).toBe('false');
		await waitFor(() => {
			expect(backend.calls.filter((c) => c.method === 'compare.count').at(-1)?.args[1]).toEqual({
				categories: ['only_in'],
				presence: [1]
			});
		});
		const presence = screen.getByRole('group', { name: 'Present on' });
		expect(
			within(presence).getByRole<HTMLInputElement>('checkbox', { name: 'Backup' }).checked
		).toBe(true);

		await user.click(await tile('Different'));
		await waitFor(() => {
			expect(
				backend.calls.filter((c) => c.method === 'compare.count').at(-1)?.args[1]
			).toMatchObject({ categories: ['different'], presence: null });
		});
	});

	it('charts bytes by extension and by size', async () => {
		compare();
		const byExtension = await screen.findByRole('table', { name: 'Bytes by extension' });
		expect(within(byExtension).getByText('jpg')).toBeTruthy();
		expect(within(byExtension).getByText('Other')).toBeTruthy();
		const bySize = await screen.findByRole('table', { name: 'Rows by size' });
		for (const bucket of ['< 1 KiB', '< 1 MiB', '< 10 MiB', '< 100 MiB', '< 1 GiB', '≥ 1 GiB']) {
			expect(within(bySize).getByText(bucket)).toBeTruthy();
		}
	});

	it('closes the comparison when leaving', async () => {
		const { backend, unmount } = compare();
		await tile('Identical');
		unmount();
		await waitFor(() => {
			expect(backend.calls.some((c) => c.method === 'compare.close')).toBe(true);
		});
	});

	it('explains a link that names no comparison', async () => {
		compare('nfc=1');
		expect(
			await screen.findByText('Choose at least two locations, or one with Find duplicates on.')
		).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Choose locations' }).getAttribute('href')).toBe(
			'/compare'
		);
	});
});

describe('CompareView filters after a refresh', () => {
	it('drops a category filter whose rows are gone', async () => {
		const user = userEvent.setup();
		const { backend } = compare();
		await user.click(await tile('Unverified'));
		backend.emitVerify(1, { type: 'ResultsChanged', summary: summary(0) });
		await waitFor(() => {
			expect(noTile('Unverified')).toBeNull();
		});
		await waitFor(() => {
			const last = backend.calls.filter((c) => c.method === 'compare.count').at(-1);
			expect(last?.args[1]).toMatchObject({ categories: [] });
		});
	});
});
