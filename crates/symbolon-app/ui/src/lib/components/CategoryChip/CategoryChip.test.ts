import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { CATEGORIES } from '../../compare/categories';
import { expectNoAxeViolations } from '../../../test/axe';
import CategoryChip from './CategoryChip.svelte';

describe('CategoryChip', () => {
	it.each(CATEGORIES.map((c) => [c.name, c.label] as const))(
		'shows %s with its label and icon',
		async (name, label) => {
			const { container } = render(CategoryChip, { category: name });
			expect(screen.getByText(label)).toBeTruthy();
			const chip = container.querySelector('[data-category]');
			expect(chip?.getAttribute('data-category')).toBe(name);
			expect(chip?.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
			await expectNoAxeViolations(container);
		}
	);

	it('keeps the label as the accessible name when compact', () => {
		render(CategoryChip, { category: 'only_in', compact: true });
		expect(screen.getByLabelText('Only in')).toBeTruthy();
		expect(screen.queryByText('Only in')).toBeNull();
	});

	it('shows a count in monospace', () => {
		render(CategoryChip, { category: 'different', count: 1234 });
		const count = screen.getByText('1,234');
		expect(count.classList.contains('sy-mono')).toBe(true);
	});
});
