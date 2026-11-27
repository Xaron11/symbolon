import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import { memorySource } from '../../data/memory';
import DataTable from './DataTable.svelte';

interface File {
	name: string;
	size: number;
}

const files = (n: number): File[] =>
	Array.from({ length: n }, (_, i) => ({ name: `file${String(i)}.jpg`, size: i * 10 }));
const columns = [
	{ id: 'name', header: 'Name', cell: (f: File) => f.name, sortable: true },
	{
		id: 'size',
		header: 'Size',
		cell: (f: File) => String(f.size),
		mono: true,
		align: 'end' as const
	}
];

async function table(rows: File[], extra: Record<string, unknown> = {}) {
	const source = memorySource(
		rows,
		(sort) => (a: File, b: File) =>
			(sort.direction === 'asc' ? 1 : -1) * a.name.localeCompare(b.name)
	);
	const result = render(DataTable<File>, { source, columns, label: 'Files', ...extra });
	const grid = screen.getByRole('grid', { name: 'Files' });
	await waitFor(() => {
		expect(grid.getAttribute('aria-rowcount')).toBe(String(rows.length + 1));
	});
	// The active row points at its cell once its page has loaded.
	await waitFor(() => {
		expect(grid.getAttribute('aria-activedescendant')).not.toBeNull();
	});
	return { ...result, grid, source };
}

const activeRow = (grid: HTMLElement) =>
	document.getElementById(grid.getAttribute('aria-activedescendant') ?? '')?.closest('[role=row]');

describe('DataTable', () => {
	it('moves with vi and arrow keys and jumps to the ends', async () => {
		const user = userEvent.setup();
		const { grid, source } = await table(files(5000));
		grid.focus();
		await user.keyboard('jj{ArrowDown}k');
		expect(activeRow(grid)?.getAttribute('aria-rowindex')).toBe('4');
		source.calls.length = 0;
		await user.keyboard('{Shift>}G{/Shift}');
		await waitFor(() => {
			expect(activeRow(grid)?.getAttribute('aria-rowindex')).toBe('5001');
		});
		expect(source.calls.every((c) => c.offset + c.limit >= 4800)).toBe(true);
		await user.keyboard('gg');
		expect(activeRow(grid)?.getAttribute('aria-rowindex')).toBe('2');
	});

	it('selects with Space, opens with Enter, moves columns with h and l', async () => {
		const user = userEvent.setup();
		const onopen = vi.fn();
		const { grid } = await table(files(10), { onopen });
		grid.focus();
		await user.keyboard('j ');
		expect(activeRow(grid)?.getAttribute('aria-selected')).toBe('true');
		await user.keyboard('{Enter}');
		expect(onopen).toHaveBeenCalledWith({ name: 'file1.jpg', size: 10 }, 1);
		await user.keyboard('l');
		expect(
			document.getElementById(grid.getAttribute('aria-activedescendant') ?? '')?.textContent
		).toBe('10');
		await user.keyboard('h');
		expect(
			document.getElementById(grid.getAttribute('aria-activedescendant') ?? '')?.textContent
		).toBe('file1.jpg');
	});

	it('asks the source to sort when a sortable header is clicked', async () => {
		const user = userEvent.setup();
		const { source } = await table(files(3));
		await user.click(screen.getByRole('button', { name: 'Name' }));
		await waitFor(() => {
			expect(source.calls.at(-1)?.sort).toEqual({ column: 'name', direction: 'asc' });
		});
		expect(screen.getByRole('columnheader', { name: 'Name' }).getAttribute('aria-sort')).toBe(
			'ascending'
		);
	});

	it('keeps long names on one line with the full text available', async () => {
		const long = 'x'.repeat(300);
		await table([{ name: long, size: 1 }]);
		const cell = screen.getByText(long);
		expect(cell.getAttribute('title')).toBe(long);
		expect(cell.classList.contains('truncate')).toBe(true);
	});

	it('has no axe violations', async () => {
		const { container } = await table(files(20));
		await expectNoAxeViolations(container);
	});

	it('sorts by the active column with s and clears the selection', async () => {
		const user = userEvent.setup();
		const { grid, source } = await table(files(5));
		grid.focus();
		await user.keyboard(' ');
		expect(activeRow(grid)?.getAttribute('aria-selected')).toBe('true');
		await user.keyboard('s');
		await waitFor(() => {
			expect(source.calls.at(-1)?.sort).toEqual({ column: 'name', direction: 'asc' });
		});
		expect(
			screen.getAllByRole('row').filter((r) => r.getAttribute('aria-selected') === 'true')
		).toEqual([]);
	});

	it('keeps focus on the grid when a row or header is clicked', async () => {
		const user = userEvent.setup();
		const { grid } = await table(files(5));
		await user.click(screen.getByText('file2.jpg'));
		expect(document.activeElement).toBe(grid);
		expect(activeRow(grid)?.getAttribute('aria-rowindex')).toBe('4');
		await user.click(screen.getByRole('button', { name: 'Name' }));
		expect(document.activeElement).toBe(grid);
	});
});

describe('DataTable live updates', () => {
	it('selection clears and the active row stays on the list when it shrinks', async () => {
		const user = userEvent.setup();
		const { grid, source } = await table(files(500));
		grid.focus();
		await user.keyboard('{Shift>}G{/Shift}');
		await user.keyboard(' ');
		await waitFor(() => {
			expect(grid.querySelectorAll('[aria-selected=true]')).toHaveLength(1);
		});
		source.replace(files(120));
		await waitFor(() => {
			expect(grid.getAttribute('aria-rowcount')).toBe('121');
		});
		await waitFor(() => {
			expect(activeRow(grid)?.getAttribute('aria-rowindex')).toBe('121');
		});
		expect(grid.querySelectorAll('[aria-selected=true]')).toHaveLength(0);
	});

	it('unsubscribes on destroy', async () => {
		const { source, unmount } = await table(files(10));
		expect(source.subscribers).toBe(1);
		unmount();
		expect(source.subscribers).toBe(0);
	});
});
