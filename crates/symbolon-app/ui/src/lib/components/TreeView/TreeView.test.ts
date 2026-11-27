import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import { memoryTree, type MemoryNode } from '../../data/memory';
import TreeView from './TreeView.svelte';

const source = () =>
	memoryTree([
		{
			key: 'photos',
			label: 'photos',
			children: [
				{
					key: 'photos/2024',
					label: '2024',
					children: [{ key: 'photos/2024/a.jpg', label: 'a.jpg' }]
				},
				{ key: 'photos/b.jpg', label: 'b.jpg' }
			]
		},
		{ key: 'notes.txt', label: 'notes.txt' }
	]);

async function tree(extra: Record<string, unknown> = {}, src = source()) {
	render(TreeView<MemoryNode>, {
		source: src,
		label: (n: MemoryNode) => n.label,
		name: 'Files',
		...extra
	});
	const view = screen.getByRole('tree', { name: 'Files' });
	await waitFor(() => {
		expect(screen.getAllByRole('treeitem').length).toBeGreaterThan(0);
	});
	return { view, src };
}

const active = (view: HTMLElement) =>
	document.getElementById(view.getAttribute('aria-activedescendant') ?? '');

describe('TreeView', () => {
	it('expands with l, moves with j/k, collapses or goes to the parent with h', async () => {
		const user = userEvent.setup();
		const { view } = await tree();
		view.focus();
		await user.keyboard('l');
		await waitFor(() => {
			expect(screen.getAllByRole('treeitem').length).toBe(4);
		});
		const photos = screen.getByRole('treeitem', { name: 'photos' });
		expect(photos.getAttribute('aria-expanded')).toBe('true');
		expect(photos.getAttribute('aria-level')).toBe('1');
		await user.keyboard('j');
		expect(active(view)?.textContent).toContain('2024');
		expect(active(view)?.getAttribute('aria-level')).toBe('2');
		expect(active(view)?.getAttribute('aria-posinset')).toBe('1');
		expect(active(view)?.getAttribute('aria-setsize')).toBe('2');
		await user.keyboard('h');
		expect(active(view)?.textContent).toContain('photos');
		await user.keyboard('h');
		expect(screen.getAllByRole('treeitem').length).toBe(2);
		expect(screen.getByRole('treeitem', { name: 'notes.txt' }).hasAttribute('aria-expanded')).toBe(
			false
		);
		await expectNoAxeViolations(view);
	});

	it('shows each full name as a tooltip and keeps focus on the tree when clicked', async () => {
		const user = userEvent.setup();
		const { view } = await tree();
		const label = screen.getByText('notes.txt');
		expect(label.getAttribute('title')).toBe('notes.txt');
		await user.click(label);
		expect(document.activeElement).toBe(view);
	});

	it('opens a leaf with Enter', async () => {
		const user = userEvent.setup();
		const onopen = vi.fn();
		const { view } = await tree({ onopen });
		view.focus();
		await user.keyboard('j{Enter}');
		expect(onopen).toHaveBeenCalledWith(expect.objectContaining({ key: 'notes.txt' }));
	});

	it('renders only the visible part of a huge folder', async () => {
		const user = userEvent.setup();
		const children: MemoryNode[] = Array.from({ length: 20_000 }, (_, i) => ({
			key: `big/${String(i)}`,
			label: `file${String(i)}`
		}));
		const { view } = await tree({}, memoryTree([{ key: 'big', label: 'big', children }]));
		view.focus();
		await user.keyboard('l');
		await waitFor(() => {
			expect(view.getAttribute('aria-activedescendant')).not.toBeNull();
		});
		expect(screen.getAllByRole('treeitem').length).toBeLessThan(100);
	});
});

describe('TreeView live updates', () => {
	it('keeps the active row by key when nodes arrive above it', async () => {
		const user = userEvent.setup();
		const src = source();
		const { view } = await tree({}, src);
		view.focus();
		await user.keyboard('j');
		expect(active(view)?.textContent).toContain('notes.txt');
		src.replace([
			{ key: 'archive', label: 'archive' },
			{ key: 'photos', label: 'photos', children: [] },
			{ key: 'notes.txt', label: 'notes.txt' }
		]);
		await waitFor(() => {
			expect(screen.getAllByRole('treeitem')).toHaveLength(3);
		});
		await waitFor(() => {
			expect(active(view)?.textContent).toContain('notes.txt');
		});
	});
});

describe('TreeView extras', () => {
	it('shows an aside beside each label', async () => {
		await tree({
			aside: (n: MemoryNode) => (n.children ? `${String(n.children.length)} items` : undefined)
		});
		expect(screen.getByText('2 items')).toBeTruthy();
	});

	it('says it is loading, then what an empty tree means', async () => {
		render(TreeView<MemoryNode>, {
			source: memoryTree([]),
			label: (n: MemoryNode) => n.label,
			name: 'Files',
			empty: 'Nothing matches the filters.'
		});
		expect(screen.getByRole('status').textContent).toContain('Loading');
		expect(await screen.findByText('Nothing matches the filters.')).toBeTruthy();
	});
});
