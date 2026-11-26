import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { CompareRowView, MemorySeed, TreeNodeView } from '../api/index.ts';
import { expectNoAxeViolations } from '../../test/axe';
import { location, renderWithBackend } from '../../test/backend';
import CompareView from './CompareView.svelte';

const row = (key: string, category: CompareRowView['category'], size: number): CompareRowView => ({
	key,
	display: key,
	category,
	presence: [0, 1],
	assumed: false,
	size,
	cells: [0, 1].map((side) => ({
		side,
		path: key,
		kind: 'File' as const,
		size,
		mtimeMs: Date.UTC(2026, 0, 2),
		hashState: 'Verified' as const,
		digestShort: 'ab12cd34ef56'
	}))
});

const folder: TreeNodeView = {
	key: 'Photos',
	name: 'Photos',
	folder: true,
	row: null,
	matchingRows: 1204,
	matchingBytes: 3.1 * 1024 ** 3
};

function seed(sides = 2): MemorySeed {
	const labels = ['Photos', 'Backup', 'Phone'].slice(0, sides);
	return {
		locations: labels.map((l) => location(l)),
		comparison: {
			summary: {
				categories: [
					{ category: 'identical', rows: 2, entries: 4, bytes: 30, assumedRows: 0 },
					{ category: 'different', rows: 1, entries: 2, bytes: 20, assumedRows: 0 }
				],
				onlyIn: []
			},
			rows: [
				row('a.jpg', 'identical', 10),
				row('b.jpg', 'different', 20),
				row('c.jpg', 'identical', 5)
			],
			tree: {
				'': [
					folder,
					{
						...folder,
						key: 'b.jpg',
						name: 'b.jpg',
						folder: false,
						row: row('b.jpg', 'different', 20),
						matchingRows: 0,
						matchingBytes: 0
					}
				],
				Photos: []
			},
			extensions: [],
			sizes: []
		}
	};
}

function open(sides = 2) {
	const labels = ['Photos', 'Backup', 'Phone'].slice(0, sides).join('%2C');
	return renderWithBackend(
		CompareView,
		{ query: new URLSearchParams(`sides=${labels}`) },
		seed(sides)
	);
}

const list = () => screen.findByRole('grid', { name: 'Rows' });

describe('drill-down', () => {
	it('lists rows with a size per side and sorts by size', async () => {
		const user = userEvent.setup();
		const { backend, container } = open();
		const grid = await list();
		await waitFor(() => {
			expect(within(grid).getByText('b.jpg')).toBeTruthy();
		});
		expect(within(grid).getByRole('columnheader', { name: 'On Backup' })).toBeTruthy();
		await user.click(within(grid).getByRole('button', { name: 'Size' }));
		await waitFor(() => {
			expect(backend.calls.filter((c) => c.method === 'compare.rows').at(-1)?.args[2]).toEqual({
				by: 'Size',
				desc: false
			});
		});
		await expectNoAxeViolations(container);
	});

	it('filters by category', async () => {
		const user = userEvent.setup();
		const { backend } = open();
		const chips = await screen.findByRole('group', { name: 'Filters' });
		await user.click(within(chips).getByRole('button', { name: 'Different' }));
		await waitFor(() => {
			expect(
				backend.calls.filter((c) => c.method === 'compare.rows').at(-1)?.args[1]
			).toMatchObject({
				categories: ['different']
			});
		});
		await waitFor(() => {
			expect(within(screen.getByRole('grid', { name: 'Rows' })).queryByText('a.jpg')).toBeNull();
		});
	});

	it('names the sides of only-in rows and the paths of moves and renames', async () => {
		const user = userEvent.setup();
		const data = seed();
		const moved = row('new/a.jpg', 'moved', 10);
		const first = moved.cells[0];
		if (first) moved.cells[0] = { ...first, path: 'old/a.jpg' };
		const onlyInBackup: CompareRowView = {
			...row('b.jpg', 'only_in', 20),
			presence: [1],
			cells: row('b.jpg', 'only_in', 20).cells.filter((c) => c.side === 1)
		};
		if (data.comparison) {
			data.comparison.rows = [moved, onlyInBackup];
			data.comparison.tree = {
				'': [
					{
						key: 'b.jpg',
						name: 'b.jpg',
						folder: false,
						row: onlyInBackup,
						matchingRows: 0,
						matchingBytes: 0
					}
				]
			};
		}
		renderWithBackend(CompareView, { query: new URLSearchParams('sides=Photos%2CBackup') }, data);
		const grid = await list();
		await waitFor(() => {
			expect(within(grid).getByText('Only in Backup')).toBeTruthy();
		});
		expect(within(grid).getByText('Moved Photos → Backup')).toBeTruthy();
		expect(within(grid).getByText('old/a.jpg → new/a.jpg')).toBeTruthy();
		await user.click(screen.getByRole('radio', { name: 'Tree' }));
		const tree = await screen.findByRole('tree', { name: 'Rows by folder' });
		await waitFor(() => {
			expect(within(tree).getByText('b.jpg — Only in Backup')).toBeTruthy();
		});
	});

	it('offers presence with two sides', async () => {
		const user = userEvent.setup();
		const { backend } = open();
		const presence = await screen.findByRole('group', { name: 'Present on' });
		await user.click(within(presence).getByRole('checkbox', { name: 'Backup' }));
		await waitFor(() => {
			expect(
				backend.calls.filter((c) => c.method === 'compare.rows').at(-1)?.args[1]
			).toMatchObject({
				presence: [1]
			});
		});
	});

	it('offers presence with three sides', async () => {
		const user = userEvent.setup();
		const { backend } = open(3);
		const presence = await screen.findByRole('group', { name: 'Present on' });
		await user.click(within(presence).getByRole('checkbox', { name: 'Phone' }));
		await waitFor(() => {
			expect(
				backend.calls.filter((c) => c.method === 'compare.rows').at(-1)?.args[1]
			).toMatchObject({
				presence: [2]
			});
		});
	});

	it('switches to the tree, which shows what lies in each folder', async () => {
		const user = userEvent.setup();
		open();
		await list();
		await user.click(screen.getByRole('radio', { name: 'Tree' }));
		const tree = await screen.findByRole('tree', { name: 'Rows by folder' });
		await waitFor(() => {
			expect(within(tree).getByText('Photos — 1,204 · 3.1 GiB')).toBeTruthy();
		});
		expect(screen.getByRole('radio', { name: 'Tree' }).getAttribute('aria-checked')).toBe('true');
	});

	it('opens a row in a side panel and closes it with Escape', async () => {
		const user = userEvent.setup();
		open();
		const grid = await list();
		await waitFor(() => {
			expect(within(grid).getByText('a.jpg')).toBeTruthy();
		});
		grid.focus();
		await user.keyboard('{Enter}');
		const panel = await screen.findByRole('complementary', { name: 'a.jpg' });
		expect(within(panel).getByText('Photos')).toBeTruthy();
		expect(within(panel).getAllByText('ab12cd34ef56')).toHaveLength(2);
		await user.keyboard('{Escape}');
		await waitFor(() => {
			expect(screen.queryByRole('complementary')).toBeNull();
		});
	});

	it('reads the rows again when verification refreshes the results', async () => {
		const { backend } = open();
		await list();
		await waitFor(() => {
			expect(backend.calls.some((c) => c.method === 'compare.count')).toBe(true);
		});
		const before = backend.calls.filter((c) => c.method === 'compare.count').length;
		backend.emitVerify(1, {
			type: 'ResultsChanged',
			summary: backend.seed.comparison?.summary ?? { categories: [], onlyIn: [] }
		});
		await waitFor(() => {
			expect(backend.calls.filter((c) => c.method === 'compare.count').length).toBeGreaterThan(
				before
			);
		});
	});
});

describe('drill-down focus', () => {
	it('returns focus to the list when the row panel closes', async () => {
		const user = userEvent.setup();
		open();
		const grid = await list();
		await waitFor(() => {
			expect(within(grid).getByText('a.jpg')).toBeTruthy();
		});
		grid.focus();
		await user.keyboard('{Enter}');
		const panel = await screen.findByRole('complementary', { name: 'a.jpg' });
		await user.click(within(panel).getByRole('button', { name: 'Close' }));
		await waitFor(() => {
			expect(document.activeElement).toBe(screen.getByRole('grid', { name: 'Rows' }));
		});
	});
});
