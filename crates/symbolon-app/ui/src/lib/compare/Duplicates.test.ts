import { screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { DuplicateSetView, MemorySeed } from '../api/index.ts';
import { expectNoAxeViolations } from '../../test/axe';
import { location, renderWithBackend } from '../../test/backend';
import CompareView from './CompareView.svelte';

const group = (n: number): DuplicateSetView => ({
	digestShort: `d${String(n).padStart(11, '0')}`,
	size: 1024 ** 2,
	redundantBytes: 2 * 1024 ** 2,
	cells: [0, 1, 2].map((i) => ({
		side: 0,
		path: `copies/${String(n)}/file${String(i)}.jpg`,
		kind: 'File' as const,
		size: 1024 ** 2,
		mtimeMs: null,
		hashState: 'Verified' as const,
		digestShort: `d${String(n).padStart(11, '0')}`
	}))
});

const seed = (): MemorySeed => ({
	locations: [location('Photos'), location('Backup')],
	comparison: {
		summary: {
			categories: [{ category: 'identical', rows: 1, entries: 2, bytes: 2, assumedRows: 0 }],
			onlyIn: []
		},
		rows: [],
		extensions: [],
		sizes: [],
		duplicates: [Array.from({ length: 130 }, (_, i) => group(i)), []],
		duplicateTotals: [
			{ groups: 130, files: 390, redundantBytes: 260 * 1024 ** 2 },
			{ groups: 0, files: 0, redundantBytes: 0 }
		]
	}
});

const open = (query: string) =>
	renderWithBackend(
		CompareView,
		{ query: new URLSearchParams(query), navigate: () => undefined },
		seed()
	);

describe('duplicates', () => {
	it('appear only when the comparison looks for them', async () => {
		open('sides=Photos%2CBackup');
		await screen.findByRole('group', { name: 'Categories' });
		expect(screen.queryByRole('heading', { name: 'Duplicates' })).toBeNull();
	});

	it('list groups per side with copies and wasted bytes, expanding to paths', async () => {
		const user = userEvent.setup();
		const { container } = open('sides=Photos%2CBackup&dupes=1');
		const inSection = within(await screen.findByRole('region', { name: 'Duplicates' }));
		expect(await inSection.findByText('130 groups · 390 files · 260 MiB redundant')).toBeTruthy();
		expect(inSection.getByRole('heading', { name: 'Photos' })).toBeTruthy();
		expect(inSection.getByText('No duplicates')).toBeTruthy();
		expect(inSection.queryByRole('tree', { name: 'Duplicates in Backup' })).toBeNull();
		const tree = await inSection.findByRole('tree', { name: 'Duplicates in Photos' });
		const first = await within(tree).findByText('copies/0/file0.jpg');
		const item = first.closest<HTMLElement>('[role=treeitem]');
		if (!item) throw new Error('a group is a tree item');
		expect(within(item).getByText('3 copies · 2.0 MiB redundant')).toBeTruthy();
		await user.click(first);
		expect(await within(tree).findByText('copies/0/file2.jpg')).toBeTruthy();
		await expectNoAxeViolations(container);
	});

	it('reach every group without a Show more button', async () => {
		const user = userEvent.setup();
		open('sides=Photos%2CBackup&dupes=1');
		const tree = await screen.findByRole('tree', { name: 'Duplicates in Photos' });
		await within(tree).findByText('copies/0/file0.jpg');
		expect(screen.queryByRole('button', { name: 'Show more' })).toBeNull();
		expect(within(tree).getAllByRole('treeitem')[0]?.getAttribute('aria-setsize')).toBe('130');
		tree.focus();
		await user.keyboard('{Shift>}G{/Shift}');
		expect(await within(tree).findByText('copies/129/file0.jpg')).toBeTruthy();
	});
});
