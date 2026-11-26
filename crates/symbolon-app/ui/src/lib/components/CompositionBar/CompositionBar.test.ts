import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { expectNoAxeViolations } from '../../../test/axe';
import CompositionBar from './CompositionBar.svelte';

const segments = [
	{ category: 'identical' as const, count: 99_990, bytes: 1024 },
	{ category: 'different' as const, count: 9 },
	{ category: 'collision' as const, count: 1 },
	{ category: 'moved' as const, count: 0 }
];

describe('CompositionBar', () => {
	it('lists every non-zero category with count and percent', async () => {
		const { container } = render(CompositionBar, { segments, label: 'Comparison' });
		const rows = screen.getAllByRole('row').slice(1);
		const cells = (row: HTMLElement) =>
			[...row.querySelectorAll('td')].map((td) => td.textContent.trim());
		expect(rows.map(cells)).toEqual([
			['Identical', '99,990', '100.0%', '1.0 KiB'],
			['Different', '9', '0.0%', '—'],
			['Collision', '1', '0.0%', '—']
		]);
		await expectNoAxeViolations(container);
	});

	it('keeps a tiny segment visible and reachable', async () => {
		const user = userEvent.setup();
		const onselect = vi.fn();
		render(CompositionBar, { segments, label: 'Comparison', onselect });
		const buttons = screen.getAllByRole('button');
		expect(buttons.map((b) => b.getAttribute('aria-label'))).toEqual([
			'Identical: 99,990 (100.0%)',
			'Different: 9 (0.0%)',
			'Collision: 1 (0.0%)'
		]);
		await user.tab();
		expect(document.activeElement).toBe(buttons[0]);
		await user.keyboard('{ArrowRight}{ArrowRight}');
		expect(document.activeElement).toBe(buttons[2]);
		await user.keyboard('{Enter}');
		expect(onselect).toHaveBeenCalledWith('collision');
		await user.keyboard('{ArrowLeft}');
		expect(document.activeElement).toBe(buttons[1]);
	});

	it('stays reachable by Tab when segments change', async () => {
		const user = userEvent.setup();
		const { rerender } = render(CompositionBar, { segments, label: 'Comparison' });
		await user.tab();
		await user.keyboard('{End}');
		await rerender({ segments: segments.slice(0, 1), label: 'Comparison' });
		expect(screen.getAllByRole('button').filter((b) => b.tabIndex === 0)).toHaveLength(1);
	});
});
